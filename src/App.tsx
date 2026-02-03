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

type Project = {
  id: string;
  name: string;
  root_path: string;
  created_at: string;
  updated_at?: string;
  google_drive_url?: string | null;
};

type Study = {
  id: string;
  project_id: string;
  internal_name: string;
  paper_label: string | null;
  status: string;
  created_at: string;
  folder_path: string;
};

type Artifact = {
  id: string;
  study_id: string;
  kind: "url" | "path";
  value: string;
  label: string | null;
  created_at: string;
};

type StudyDetail = {
  study: Study;
  artifacts: Artifact[];
};

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [studies, setStudies] = useState<Study[]>([]);
  const [selectedStudyId, setSelectedStudyId] = useState<string | null>(null);
  const [detail, setDetail] = useState<StudyDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
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

  const selectedStudy = useMemo(
    () => studies.find((s) => s.id === selectedStudyId) ?? null,
    [studies, selectedStudyId]
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
    const loadStudies = async () => {
      if (!selectedProjectId) {
        setStudies([]);
        setSelectedStudyId(null);
        setDetail(null);
        return;
      }
      try {
        setLoading(true);
        const list = await invoke<Study[]>("list_studies", {
          project_id: selectedProjectId
        });
        setStudies(list);
        setSelectedStudyId(list[0]?.id ?? null);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadStudies();
  }, [selectedProjectId]);

  useEffect(() => {
    const loadDetail = async () => {
      if (!selectedStudyId) {
        setDetail(null);
        return;
      }
      try {
        setLoading(true);
      const detail = await invoke<StudyDetail>("get_study_detail", {
        study_id: selectedStudyId
      });
        setDetail(detail);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadDetail();
  }, [selectedStudyId]);

  const refreshProjects = async (selectId?: string) => {
    const list = await invoke<Project[]>("list_projects");
    setProjects(list);
    if (selectId) {
      setSelectedProjectId(selectId);
    }
  };

  const refreshStudies = async (projectId: string, selectId?: string) => {
    const list = await invoke<Study[]>("list_studies", { project_id: projectId });
    setStudies(list);
    if (selectId) {
      setSelectedStudyId(selectId);
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
        name: trimmedName,
        root_dir: trimmedRoot,
        google_drive_url: trimmedDrive ? trimmedDrive : null
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
    if (!selectedProjectId) return;
    const internalName = window.prompt("Study internal name?");
    if (!internalName) return;
    const paperLabel = window.prompt("Optional paper label? (leave blank for none)") || null;
    try {
      setLoading(true);
      const study = await invoke<Study>("create_study", {
        project_id: selectedProjectId,
        internal_name: internalName,
        paper_label: paperLabel
      });
      await refreshStudies(selectedProjectId, study.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRenameStudy = async () => {
    if (!detail) return;
    const internalName = window.prompt(
      "New internal name?",
      detail.study.internal_name
    );
    if (!internalName) return;
    const paperLabel = window.prompt(
      "Paper label? (leave blank for none)",
      detail.study.paper_label ?? ""
    );
    try {
      setLoading(true);
      await invoke("rename_study", {
        study_id: detail.study.id,
        internal_name: internalName,
        paper_label: paperLabel?.trim() ? paperLabel.trim() : null
      });
      await refreshStudies(detail.study.project_id, detail.study.id);
      const updated = await invoke<StudyDetail>("get_study_detail", {
        study_id: detail.study.id
      });
      setDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleUpdateStatus = async (status: string) => {
    if (!detail) return;
    try {
      setLoading(true);
    await invoke("update_study_status", { study_id: detail.study.id, status });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        study_id: detail.study.id
      });
      setDetail(updated);
      await refreshStudies(detail.study.project_id, detail.study.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAddArtifact = async () => {
    if (!detail) return;
    const kindInput = window.prompt("Artifact type: url or path?", "url");
    if (!kindInput) return;
    const kind = kindInput.toLowerCase() === "path" ? "path" : "url";
    const value = window.prompt("Value (URL or local file path)?");
    if (!value) return;
    const label = window.prompt("Optional label? (leave blank for none)") || null;
    try {
      setLoading(true);
      await invoke("add_artifact", {
        study_id: detail.study.id,
        kind,
        value,
        label
      });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        study_id: detail.study.id
      });
      setDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRemoveArtifact = async (artifactId: string) => {
    if (!detail) return;
    if (!window.confirm("Remove this artifact?") ) return;
    try {
      setLoading(true);
      await invoke("remove_artifact", { artifact_id: artifactId });
      const updated = await invoke<StudyDetail>("get_study_detail", {
        study_id: detail.study.id
      });
      setDetail(updated);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleGenerateOsf = async () => {
    if (!detail) return;
    const includePilots = window.confirm(
      "Include pilot studies?\nOK = Include pilots\nCancel = Exclude pilots (default)"
    );
    try {
      setLoading(true);
      const result = await invoke<string>("generate_osf_packages", {
        study_id: detail.study.id,
        include_pilots: includePilots
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

  return (
    <div className="app">
      <header className="app-header">
        <div>
          <h1>Research Workflow</h1>
          <p>Local-first project + study manager.</p>
        </div>
        <div className="header-actions">
          <button onClick={handleGitStatus}>Git Status</button>
          <button onClick={handleGitCommitPush}>Commit + Push</button>
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
                    <span>{project.root_path}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="panel">
          <div className="panel-header">
            <h2>Studies</h2>
            <button onClick={handleAddStudy} disabled={!selectedProjectId}>
              Add Study
            </button>
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
                    <strong>{study.internal_name}</strong>
                    <span>{study.paper_label ?? "(no paper label)"}</span>
                    <span className="pill">{study.status}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="panel detail">
          <div className="panel-header">
            <h2>Study Detail</h2>
            <div className="inline-actions">
              <button onClick={handleRenameStudy} disabled={!detail}>
                Rename Study
              </button>
              <button onClick={handleGenerateOsf} disabled={!detail}>
                Generate OSF Packages
              </button>
            </div>
          </div>
          <div className="panel-body">
            {!detail && <p className="muted">Select a study.</p>}
            {detail && (
              <div className="detail-content">
                <div className="detail-row">
                  <div>
                    <h3>{detail.study.internal_name}</h3>
                    <p className="muted">Stable ID: {detail.study.id}</p>
                    <p className="muted">Folder: {detail.study.folder_path}</p>
                  </div>
                  <div>
                    <label>Status</label>
                    <select
                      value={detail.study.status}
                      onChange={(event) => handleUpdateStatus(event.target.value)}
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
                    <button onClick={handleAddArtifact}>Add Artifact</button>
                  </div>
                  {detail.artifacts.length === 0 && (
                    <p className="muted">No artifacts yet.</p>
                  )}
                  <ul className="artifact-list">
                    {detail.artifacts.map((artifact) => (
                      <li key={artifact.id}>
                        <div>
                          <strong>{artifact.label ?? artifact.kind}</strong>
                          <span>{artifact.value}</span>
                        </div>
                        <button onClick={() => handleRemoveArtifact(artifact.id)}>
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
