import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";

const STATUSES = [
  "planning",
  "collecting",
  "analyzing",
  "writing",
  "submitted",
  "published",
  "archived"
];

const STUDY_CODE_PATTERN = /^S-[A-Z0-9]{6}$/;

type Project = {
  id: string;
  name: string;
  rootPath: string;
  createdAt: string;
  updatedAt?: string;
  googleDriveUrl?: string | null;
  studies: JsonStudy[];
};

type JsonStudy = {
  id: string;
  createdAt: string;
  title: string;
  folderPath?: string;
};

type LegacyStudy = {
  id: string;
  projectId: string;
  internalName: string;
  paperLabel: string | null;
  status: string;
  createdAt: string;
  folderPath: string;
};

type Artifact = {
  id: string;
  studyId: string;
  kind: "url" | "path";
  value: string;
  label: string | null;
  createdAt: string;
};

type StudyDetail = {
  study: LegacyStudy;
  artifacts: Artifact[];
};

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [selectedStudyId, setSelectedStudyId] = useState<string | null>(null);
  const [legacyStudies, setLegacyStudies] = useState<LegacyStudy[]>([]);
  const [selectedLegacyStudyId, setSelectedLegacyStudyId] = useState<string | null>(null);
  const [legacyDetail, setLegacyDetail] = useState<StudyDetail | null>(null);
  const [showLegacy, setShowLegacy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [addStudyClickCount, setAddStudyClickCount] = useState(0);
  const [addStudyDebug, setAddStudyDebug] = useState<string | null>(null);
  const [isEditingStudyTitle, setIsEditingStudyTitle] = useState(false);
  const [studyTitleDraft, setStudyTitleDraft] = useState("");
  const [isProjectModalOpen, setIsProjectModalOpen] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [projectRoot, setProjectRoot] = useState("");
  const [googleDriveUrl, setGoogleDriveUrl] = useState("");
  const [projectFormErrors, setProjectFormErrors] = useState<{
    name?: string;
    root?: string;
  }>({});

  const selectedProject = useMemo(
    () => projects.find((p) => p.id === selectedProjectId) ?? null,
    [projects, selectedProjectId]
  );

  const studies = useMemo(
    () => selectedProject?.studies ?? [],
    [selectedProject]
  );

  const selectedStudy = useMemo(
    () => studies.find((s) => s.id === selectedStudyId) ?? null,
    [studies, selectedStudyId]
  );

  useEffect(() => {
    if (!selectedStudy) {
      setIsEditingStudyTitle(false);
      setStudyTitleDraft("");
      return;
    }
    setIsEditingStudyTitle(false);
    setStudyTitleDraft(selectedStudy.title);
  }, [selectedStudy]);

  const selectedLegacyStudy = useMemo(
    () => legacyStudies.find((s) => s.id === selectedLegacyStudyId) ?? null,
    [legacyStudies, selectedLegacyStudyId]
  );

  useEffect(() => {
    const init = async () => {
      try {
        setLoading(true);
        await invoke("init_db");
        const list = await invoke<Project[]>("list_projects");
        setProjects(list);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    init();
  }, []);

  useEffect(() => {
    if (!selectedProjectId) {
      setSelectedStudyId(null);
      return;
    }
    if (studies.length === 0) {
      setSelectedStudyId(null);
      return;
    }
    if (!studies.some((study) => study.id === selectedStudyId)) {
      setSelectedStudyId(studies[0].id);
    }
  }, [selectedProjectId, selectedStudyId, studies]);

  useEffect(() => {
    const loadLegacyStudies = async () => {
      if (!selectedProjectId || !showLegacy) {
        setLegacyStudies([]);
        setSelectedLegacyStudyId(null);
        setLegacyDetail(null);
        return;
      }
      try {
        setLoading(true);
        const list = await invoke<LegacyStudy[]>("list_studies", {
          args: { projectId: selectedProjectId }
        });
        setLegacyStudies(list);
        setSelectedLegacyStudyId(list[0]?.id ?? null);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadLegacyStudies();
  }, [selectedProjectId, showLegacy]);

  useEffect(() => {
    const loadLegacyDetail = async () => {
      if (!selectedLegacyStudyId || !showLegacy) {
        setLegacyDetail(null);
        return;
      }
      try {
        setLoading(true);
        const detail = await invoke<StudyDetail>("get_study_detail", {
          args: { studyId: selectedLegacyStudyId }
        });
        setLegacyDetail(detail);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadLegacyDetail();
  }, [selectedLegacyStudyId, showLegacy]);

  const refreshProjects = async (selectId?: string) => {
    const list = await invoke<Project[]>("list_projects");
    setProjects(list);
    if (selectId) {
      setSelectedProjectId(selectId);
    }
  };

  const resetProjectModal = () => {
    setProjectName("");
    setProjectRoot("");
    setGoogleDriveUrl("");
    setProjectFormErrors({});
  };

  const openProjectModal = () => {
    resetProjectModal();
    setIsProjectModalOpen(true);
  };

  const closeProjectModal = () => {
    setIsProjectModalOpen(false);
  };

  const handlePickProjectRoot = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select project root location"
      });
      if (typeof selected === "string") {
        setProjectRoot(selected);
        setProjectFormErrors((prev) => ({ ...prev, root: undefined }));
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleCreateProject = async () => {
    const trimmedName = projectName.trim();
    const trimmedRoot = projectRoot.trim();
    const trimmedDrive = googleDriveUrl.trim();
    const nextErrors: { name?: string; root?: string } = {};

    if (!trimmedName) {
      nextErrors.name = "Project name is required.";
    }
    if (!trimmedRoot) {
      nextErrors.root = "Project root location is required.";
    }

    setProjectFormErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) return;

    try {
      setLoading(true);
      const project = await invoke<Project>("create_project", {
        args: {
          name: trimmedName,
          rootDir: trimmedRoot,
          googleDriveUrl: trimmedDrive ? trimmedDrive : null
        }
      });
      await refreshProjects(project.id);
      closeProjectModal();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAddStudy = async () => {
    setAddStudyClickCount((prev) => prev + 1);
    if (!selectedProjectId) return;
    try {
      setError(null);
      setLoading(true);
      setAddStudyDebug(
        `invoke add_study: projectId=${selectedProjectId} folderName=auto`
      );
      const project = await invoke<Project>("add_study", {
        args: {
          projectId: selectedProjectId,
          folderName: "",
          title: null
        }
      });
      setAddStudyDebug(
        `add_study success: projectId=${project.id} studies=${project.studies.length}`
      );
      setProjects((prev) => {
        const index = prev.findIndex((item) => item.id === project.id);
        if (index === -1) {
          return [project, ...prev];
        }
        return prev.map((item) => (item.id === project.id ? project : item));
      });
      const lastStudy = project.studies[project.studies.length - 1];
      setSelectedStudyId(lastStudy?.id ?? null);
    } catch (err) {
      setAddStudyDebug(`add_study error: ${String(err)}`);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRenameJsonStudy = async () => {
    if (!selectedProject || !selectedStudy) return;
    const title = studyTitleDraft.trim();
    if (!title) {
      setError("Study title is required.");
      return;
    }
    try {
      setLoading(true);
      const project = await invoke<Project>("rename_study_json", {
        args: {
          projectId: selectedProject.id,
          studyId: selectedStudy.id,
          title
        }
      });
      setProjects((prev) =>
        prev.map((item) => (item.id === project.id ? project : item))
      );
      setSelectedStudyId(selectedStudy.id);
      setIsEditingStudyTitle(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRenameJsonFolder = async () => {
    if (!selectedProject || !selectedStudy) return;
    const folderName = window.prompt(
      "New study folder name? (e.g., S-7F3A9C)",
      selectedStudy.id
    );
    if (folderName === null) return;
    const normalizedFolder = folderName.trim().toUpperCase();
    if (!STUDY_CODE_PATTERN.test(normalizedFolder)) {
      setError("Study folder name must match S-XXXXXX (letters/numbers).");
      return;
    }
    try {
      setLoading(true);
      const project = await invoke<Project>("rename_study_folder_json", {
        args: {
          projectId: selectedProject.id,
          studyId: selectedStudy.id,
          folderName: normalizedFolder
        }
      });
      setProjects((prev) =>
        prev.map((item) => (item.id === project.id ? project : item))
      );
      setSelectedStudyId(normalizedFolder);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const refreshLegacyStudies = async (projectId: string, selectId?: string) => {
    const list = await invoke<LegacyStudy[]>("list_studies", {
      args: { projectId }
    });
    setLegacyStudies(list);
    if (selectId) {
      setSelectedLegacyStudyId(selectId);
    }
  };

  const handleRenameLegacyStudy = async () => {
    if (!legacyDetail) return;
    const internalName = window.prompt(
      "New internal name?",
      legacyDetail.study.internalName
    );
    if (!internalName) return;
    const paperLabel = window.prompt(
      "Paper label? (leave blank for none)",
      legacyDetail.study.paperLabel ?? ""
    );
    try {
      setLoading(true);
      await invoke("rename_study", {
        args: {
          studyId: legacyDetail.study.id,
          internalName: internalName,
          paperLabel: paperLabel?.trim() ? paperLabel.trim() : null
        }
      });
      await refreshLegacyStudies(legacyDetail.study.projectId, legacyDetail.study.id);
      const updated = await invoke<StudyDetail>("get_study_detail", {
        args: { studyId: legacyDetail.study.id }
      });
      setLegacyDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleUpdateLegacyStatus = async (status: string) => {
    if (!legacyDetail) return;
    try {
      setLoading(true);
      await invoke("update_study_status", {
        args: { studyId: legacyDetail.study.id, status }
      });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        args: { studyId: legacyDetail.study.id }
      });
      setLegacyDetail(updated);
      await refreshLegacyStudies(legacyDetail.study.projectId, legacyDetail.study.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAddLegacyArtifact = async () => {
    if (!legacyDetail) return;
    const kindInput = window.prompt("Artifact type: url or path?", "url");
    if (!kindInput) return;
    const kind = kindInput.toLowerCase() === "path" ? "path" : "url";
    const value = window.prompt("Value (URL or local file path)?");
    if (!value) return;
    const label = window.prompt("Optional label? (leave blank for none)") || null;
    try {
      setLoading(true);
      await invoke("add_artifact", {
        args: {
          studyId: legacyDetail.study.id,
          kind,
          value,
          label
        }
      });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        args: { studyId: legacyDetail.study.id }
      });
      setLegacyDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRemoveLegacyArtifact = async (artifactId: string) => {
    if (!legacyDetail) return;
    if (!window.confirm("Remove this artifact?")) return;
    try {
      setLoading(true);
      await invoke("remove_artifact", { args: { artifactId } });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        args: { studyId: legacyDetail.study.id }
      });
      setLegacyDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleGenerateLegacyOsf = async () => {
    if (!legacyDetail) return;
    const includePilots = window.confirm(
      "Include pilot studies?\nOK = Include pilots\nCancel = Exclude pilots (default)"
    );
    try {
      setLoading(true);
      const result = await invoke<string>("generate_osf_packages", {
        args: {
          studyId: legacyDetail.study.id,
          includePilots: includePilots
        }
      });
      alert(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };
  const handleGitStatus = async () => {
    try {
      setLoading(true);
      const output = await invoke<string>("git_status");
      alert(output);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleGitCommitPush = async () => {
    const message = window.prompt("Commit message?");
    if (!message) return;
    try {
      setLoading(true);
      const output = await invoke<string>("git_commit_push", { message });
      alert(output);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleMigrateJsonToSqlite = async () => {
    try {
      setLoading(true);
      const output = await invoke<string>("migrate_json_to_sqlite");
      alert(output);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="app">
      <header className="app-header">
        <div>
          <h1>Research Workflow DEV3</h1>
          <p>Local-first project + study manager.</p>
          <p className="muted">Add Study clicks: {addStudyClickCount}</p>
          <p className="muted">
            Add Study debug: {addStudyDebug ?? "(none yet)"}
          </p>
        </div>
        <div className="header-actions">
          <button onClick={handleGitStatus}>Git Status</button>
          <button onClick={handleGitCommitPush}>Commit + Push</button>
          <button onClick={handleMigrateJsonToSqlite}>
            Migrate JSON → SQLite
          </button>
        </div>
      </header>

      {error && (
        <div className="error">
          {error}
          <button onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      <div className="layout">
        <section className="panel">
          <div className="panel-header">
            <h2>Projects</h2>
            <button onClick={openProjectModal}>New Project</button>
          </div>
          <div className="panel-body">
            {projects.length === 0 && <p className="muted">No projects yet.</p>}
            <ul className="list">
              {projects.map((project) => (
                <li key={project.id}>
                  <button
                    className={project.id === selectedProjectId ? "active" : ""}
                    onClick={() => setSelectedProjectId(project.id)}
                  >
                    <strong>{project.name}</strong>
                    <span>{project.rootPath}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="panel">
          <div className="panel-header">
            <h2>Studies</h2>
            <div className="inline-actions">
              <button onClick={handleAddStudy} disabled={!selectedProjectId}>
                Add Study
              </button>
              <button onClick={() => setShowLegacy((prev) => !prev)}>
                {showLegacy ? "Hide Legacy" : "Show Legacy"}
              </button>
            </div>
          </div>
          <div className="panel-body">
            {!selectedProjectId && <p className="muted">Select a project.</p>}
            {selectedProjectId && studies.length === 0 && (
              <p className="muted">No studies yet.</p>
            )}
            <ul className="list">
              {studies.map((study) => (
                <li key={study.id}>
                  <button
                    className={study.id === selectedStudyId ? "active" : ""}
                    onClick={() => setSelectedStudyId(study.id)}
                  >
                    <strong>{study.title}</strong>
                    <span>{study.createdAt}</span>
                  </button>
                </li>
              ))}
            </ul>

            {showLegacy && (
              <>
                <div className="divider" />
                <p className="muted">Legacy (SQLite) studies</p>
                {selectedProjectId && legacyStudies.length === 0 && (
                  <p className="muted">No legacy studies yet.</p>
                )}
                <ul className="list">
                  {legacyStudies.map((study) => (
                    <li key={study.id}>
                      <button
                        className={study.id === selectedLegacyStudyId ? "active" : ""}
                        onClick={() => setSelectedLegacyStudyId(study.id)}
                      >
                        <strong>{study.internalName}</strong>
                        <span>{study.paperLabel ?? "(no paper label)"}</span>
                        <span className="pill">{study.status}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </section>

        <section className="panel detail">
          <div className="panel-header">
            <h2>Study Detail</h2>
          </div>
          <div className="panel-body">
            {!selectedStudy && <p className="muted">Select a study.</p>}
            {selectedStudy && (
              <div className="detail-content">
                <div className="detail-row">
                  <div>
                    <h3>{selectedStudy.title}</h3>
                    <p className="muted">Stable ID: {selectedStudy.id}</p>
                    <p className="muted">Created: {selectedStudy.createdAt}</p>
                    {selectedStudy.folderPath && (
                      <p className="muted">Folder: {selectedStudy.folderPath}</p>
                    )}
                  </div>
                  <div>
                    {!isEditingStudyTitle && (
                      <button
                        onClick={() => {
                          setError(null);
                          setStudyTitleDraft(selectedStudy.title);
                          setIsEditingStudyTitle(true);
                        }}
                      >
                        Rename Study
                      </button>
                    )}
                    <button onClick={handleRenameJsonFolder}>
                      Rename Folder
                    </button>
                  </div>
                </div>
                {isEditingStudyTitle && (
                  <div className="detail-row">
                    <div>
                      <label>Study Title</label>
                      <input
                        value={studyTitleDraft}
                        onChange={(event) => setStudyTitleDraft(event.target.value)}
                      />
                      {!studyTitleDraft.trim() && (
                        <p className="muted">Title is required.</p>
                      )}
                    </div>
                    <div>
                      <button
                        onClick={handleRenameJsonStudy}
                        disabled={!studyTitleDraft.trim()}
                      >
                        Save
                      </button>
                      <button
                        className="ghost"
                        onClick={() => setIsEditingStudyTitle(false)}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </section>

        {showLegacy && (
          <section className="panel detail">
            <div className="panel-header">
              <h2>Legacy Study Detail</h2>
              <div className="inline-actions">
                <button onClick={handleRenameLegacyStudy} disabled={!legacyDetail}>
                  Rename Study
                </button>
                <button onClick={handleGenerateLegacyOsf} disabled={!legacyDetail}>
                  Generate OSF Packages
                </button>
              </div>
            </div>
            <div className="panel-body">
              {!legacyDetail && <p className="muted">Select a legacy study.</p>}
              {legacyDetail && (
                <div className="detail-content">
                  <div className="detail-row">
                    <div>
                      <h3>{legacyDetail.study.internalName}</h3>
                      <p className="muted">Stable ID: {legacyDetail.study.id}</p>
                      <p className="muted">Folder: {legacyDetail.study.folderPath}</p>
                    </div>
                    <div>
                      <label>Status</label>
                      <select
                        value={legacyDetail.study.status}
                        onChange={(event) => handleUpdateLegacyStatus(event.target.value)}
                      >
                        {STATUSES.map((status) => (
                          <option key={status} value={status}>
                            {status}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  <div className="artifacts">
                    <div className="panel-header compact">
                      <h3>Artifacts</h3>
                      <button onClick={handleAddLegacyArtifact}>Add Artifact</button>
                    </div>
                    {legacyDetail.artifacts.length === 0 && (
                      <p className="muted">No artifacts yet.</p>
                    )}
                    <ul className="artifact-list">
                      {legacyDetail.artifacts.map((artifact) => (
                        <li key={artifact.id}>
                          <div>
                            <strong>{artifact.label ?? artifact.kind}</strong>
                            <span>{artifact.value}</span>
                          </div>
                          <button onClick={() => handleRemoveLegacyArtifact(artifact.id)}>
                            Remove
                          </button>
                        </li>
                      ))}
                    </ul>
                  </div>
                </div>
              )}
            </div>
          </section>
        )}
      </div>

      {loading && <div className="loading">Working...</div>}

      {isProjectModalOpen && (
        <div className="modal-backdrop" onClick={closeProjectModal}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h2>New Project</h2>
              <button className="ghost" onClick={closeProjectModal}>
                Close
              </button>
            </div>
            <div className="modal-body">
              <label htmlFor="project-name">Project Name *</label>
              <input
                id="project-name"
                value={projectName}
                onChange={(event) => {
                  setProjectName(event.target.value);
                  if (projectFormErrors.name) {
                    setProjectFormErrors((prev) => ({ ...prev, name: undefined }));
                  }
                }}
                placeholder="e.g., Distraction & Memory"
              />
              {projectFormErrors.name && (
                <p className="field-error">{projectFormErrors.name}</p>
              )}

              <label>Project Root Location *</label>
              <div className="inline-field">
                <input
                  value={projectRoot}
                  placeholder="Choose a folder"
                  readOnly
                />
                <button onClick={handlePickProjectRoot}>Choose</button>
              </div>
              {projectFormErrors.root && (
                <p className="field-error">{projectFormErrors.root}</p>
              )}

              <label htmlFor="drive-url">Google Drive Folder URL (optional)</label>
              <input
                id="drive-url"
                value={googleDriveUrl}
                onChange={(event) => setGoogleDriveUrl(event.target.value)}
                placeholder="https://drive.google.com/..."
              />
            </div>
            <div className="modal-actions">
              <button className="ghost" onClick={closeProjectModal}>
                Cancel
              </button>
              <button onClick={handleCreateProject}>Create Project</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
