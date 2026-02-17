import { useEffect, useMemo, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import { AnalysisTemplateWizard } from "./components/AnalysisTemplateWizard";
import { AnalysisTemplateOptions } from "./types/analysisTemplate";

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
  analysisPackageDefaults?: {
    cleaning: string[];
    plot: string[];
    table: string[];
    analysis: string[];
  } | null;
  studies: JsonStudy[];
};

type FileRef = {
  path: string;
  name: string;
  kind: string;
};

type JsonStudy = {
  id: string;
  createdAt: string;
  title: string;
  folderPath?: string;
  files?: FileRef[];
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

type RootDirInfo = {
  exists: boolean;
  isGitRepo: boolean;
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
  const [projectRootMode, setProjectRootMode] = useState<"new" | "existing">("new");
  const [projectRootInfo, setProjectRootInfo] = useState<RootDirInfo | null>(
    null
  );
  const [selectedRootInfo, setSelectedRootInfo] = useState<RootDirInfo | null>(
    null
  );
  const [isProjectSettingsOpen, setIsProjectSettingsOpen] = useState(false);
  const [projectRootEdit, setProjectRootEdit] = useState("");
  const [projectRootEditInfo, setProjectRootEditInfo] = useState<RootDirInfo | null>(
    null
  );
  const [projectRootEditError, setProjectRootEditError] = useState<string | null>(
    null
  );
  const [deleteProjectOnDisk, setDeleteProjectOnDisk] = useState(false);
  const [deleteStudyOnDisk, setDeleteStudyOnDisk] = useState(false);
  const [isAnalysisModalOpen, setIsAnalysisModalOpen] = useState(false);
  const [analysisTarget, setAnalysisTarget] = useState<{
    projectId: string;
    studyId: string;
  } | null>(null);
  const [isRemoveAnalysisModalOpen, setIsRemoveAnalysisModalOpen] = useState(false);
  const [removeAnalysisTarget, setRemoveAnalysisTarget] = useState<{
    projectId: string;
    studyId: string;
  } | null>(null);
  const [analysisFiles, setAnalysisFiles] = useState<string[]>([]);
  const [studyTab, setStudyTab] = useState<"overview" | "files" | "danger">(
    "overview"
  );
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
      setStudyTab("overview");
      return;
    }
    setIsEditingStudyTitle(false);
    setStudyTitleDraft(selectedStudy.title);
    setStudyTab("overview");
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
    setProjectRootMode("new");
    setProjectRootInfo(null);
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

  const openProjectSettings = () => {
    if (!selectedProject) return;
    setProjectRootEdit(selectedProject.rootPath);
    setProjectRootEditError(null);
    setProjectRootEditInfo(null);
    setDeleteProjectOnDisk(false);
    setIsProjectSettingsOpen(true);
  };

  const closeProjectSettings = () => {
    setIsProjectSettingsOpen(false);
  };

  const handlePickProjectRoot = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title:
          projectRootMode === "existing"
            ? "Select existing project folder"
            : "Select parent folder for new project"
      });
      if (typeof selected === "string") {
        setProjectRoot(selected);
        setProjectFormErrors((prev) => ({ ...prev, root: undefined }));
        try {
          const info = await invoke<RootDirInfo>("check_root_dir", {
            rootDir: selected
          });
          setProjectRootInfo(info);
        } catch (err) {
          setError(String(err));
        }
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handlePickProjectRootEdit = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select project root folder"
      });
      if (typeof selected === "string") {
        setProjectRootEdit(selected);
        setProjectRootEditError(null);
        try {
          const info = await invoke<RootDirInfo>("check_root_dir", {
            rootDir: selected
          });
          setProjectRootEditInfo(info);
        } catch (err) {
          setProjectRootEditError(String(err));
        }
      }
    } catch (err) {
      setProjectRootEditError(String(err));
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
          useExistingRoot: projectRootMode === "existing",
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

  const handleUpdateProjectRoot = async () => {
    if (!selectedProject) return;
    const trimmedRoot = projectRootEdit.trim();
    if (!trimmedRoot) {
      setProjectRootEditError("Project root location is required.");
      return;
    }
    try {
      setLoading(true);
      const project = await invoke<Project>("update_project_root", {
        args: {
          projectId: selectedProject.id,
          rootDir: trimmedRoot
        }
      });
      setProjects((prev) =>
        prev.map((item) => (item.id === project.id ? project : item))
      );
      setSelectedProjectId(project.id);
      setIsProjectSettingsOpen(false);
    } catch (err) {
      setProjectRootEditError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteProject = async () => {
    if (!selectedProject) return;
    const confirmMessage = deleteProjectOnDisk
      ? `Delete project "${selectedProject.name}" and permanently remove its folder on disk?\n\nFolder:\n${selectedProject.rootPath}`
      : `Delete project "${selectedProject.name}" from the app?\nThis does not delete files on disk.`;
    if (!window.confirm(confirmMessage)) return;
    try {
      setLoading(true);
      await invoke("delete_project", {
        args: {
          projectId: selectedProject.id,
          deleteOnDisk: deleteProjectOnDisk
        }
      });
      await refreshProjects();
      setSelectedProjectId(null);
      setSelectedStudyId(null);
      setIsProjectSettingsOpen(false);
    } catch (err) {
      setProjectRootEditError(String(err));
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

  const handleDeleteStudy = async () => {
    if (!selectedProject || !selectedStudy) return;
    const confirmation = window.prompt(
      `Type the study ID to confirm deletion: ${selectedStudy.id}`,
      ""
    );
    if (confirmation === null) return;
    if (confirmation.trim() !== selectedStudy.id) {
      setError("Study ID did not match. Deletion cancelled.");
      return;
    }
    const confirmMessage = deleteStudyOnDisk
      ? `Delete study "${selectedStudy.title}" and remove its folder on disk?`
      : `Delete study "${selectedStudy.title}" from the project?`;
    if (!window.confirm(confirmMessage)) return;
    try {
      setLoading(true);
      const project = await invoke<Project>("delete_study", {
        args: {
          projectId: selectedProject.id,
          studyId: selectedStudy.id,
          deleteOnDisk: deleteStudyOnDisk
        }
      });
      setProjects((prev) =>
        prev.map((item) => (item.id === project.id ? project : item))
      );
      setSelectedStudyId(project.studies[0]?.id ?? null);
      setDeleteStudyOnDisk(false);
    } catch (err) {
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

  const handleImportFiles = async () => {
    if (!selectedProject || !selectedStudy) return;
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        title: "Import files into study"
      });
      const paths = Array.isArray(selected)
        ? selected.filter(Boolean)
        : typeof selected === "string"
          ? [selected]
          : [];
      if (paths.length === 0) return;
      setLoading(true);
      const updatedStudy = await invoke<JsonStudy>("import_files", {
        projectId: selectedProject.id,
        studyId: selectedStudy.id,
        paths
      });
      setProjects((prev) =>
        prev.map((project) =>
          project.id === selectedProject.id
            ? {
                ...project,
                studies: project.studies.map((study) =>
                  study.id === updatedStudy.id ? updatedStudy : study
                )
              }
            : project
        )
      );
      setSelectedStudyId(updatedStudy.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAddAnalysis = async (
    projectId: string,
    studyId: string,
    options: AnalysisTemplateOptions
  ) => {
    try {
      setError(null);
      setLoading(true);
      const message = await invoke<string>("create_analysis_template", {
        projectId,
        studyId,
        options
      });
      alert(message);
      closeAnalysisModal();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const openAnalysisModal = (projectId: string, studyId: string) => {
    setError(null);
    setAnalysisTarget({ projectId, studyId });
    setIsAnalysisModalOpen(true);
  };

  const openRemoveAnalysisModal = async (projectId: string, studyId: string) => {
    try {
      setLoading(true);
      const files = await invoke<string[]>("list_analysis_templates", {
        args: { projectId, studyId }
      });
      setAnalysisFiles(files);
      setRemoveAnalysisTarget({ projectId, studyId });
      setIsRemoveAnalysisModalOpen(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const closeRemoveAnalysisModal = () => {
    setIsRemoveAnalysisModalOpen(false);
    setRemoveAnalysisTarget(null);
    setAnalysisFiles([]);
  };

  const handleRemoveAnalysis = async (
    projectId: string,
    studyId: string,
    name: string
  ) => {
    if (!window.confirm(`Delete analysis "${name}.Rmd" from this study?`)) return;
    try {
      setLoading(true);
      const message = await invoke<string>("delete_analysis_template", {
        args: {
          projectId,
          studyId,
          analysisName: name.trim()
        }
      });
      setAnalysisFiles((prev) => prev.filter((item) => item !== name));
      alert(message);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const closeAnalysisModal = () => {
    setIsAnalysisModalOpen(false);
    setAnalysisTarget(null);
  };

  const handleRemoveFile = async (path: string) => {
    if (!selectedProject || !selectedStudy) return;
    if (!window.confirm("Remove this file from the study and delete it from disk?")) {
      return;
    }
    try {
      setLoading(true);
      const updatedStudy = await invoke<JsonStudy>("remove_file_ref", {
        projectId: selectedProject.id,
        studyId: selectedStudy.id,
        path
      });
      setProjects((prev) =>
        prev.map((project) =>
          project.id === selectedProject.id
            ? {
                ...project,
                studies: project.studies.map((study) =>
                  study.id === updatedStudy.id ? updatedStudy : study
                )
              }
            : project
        )
      );
      setSelectedStudyId(updatedStudy.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const buildFilePath = (rootPath: string, relPath: string) => {
    const separator = rootPath.includes("\\") ? "\\" : "/";
    const root = rootPath.replace(/[\\/]+$/, "");
    const rel = relPath.replace(/[\\/]+/g, separator);
    return `${root}${separator}${rel}`;
  };

  const isImageKind = (kind: string) => ["png", "jpg"].includes(kind);

  useEffect(() => {
    const loadRootInfo = async () => {
      if (!selectedProject) {
        setSelectedRootInfo(null);
        return;
      }
      try {
        const info = await invoke<RootDirInfo>("check_root_dir", {
          rootDir: selectedProject.rootPath
        });
        setSelectedRootInfo(info);
      } catch (err) {
        setSelectedRootInfo(null);
      }
    };
    loadRootInfo();
  }, [selectedProject?.rootPath]);

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
            <div className="inline-actions">
              <button onClick={openProjectModal}>New Project</button>
              <button onClick={openProjectSettings} disabled={!selectedProject}>
                Project Settings
              </button>
            </div>
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
            {selectedProjectId && selectedProject && (
              <div className="project-meta">
                <p className="muted">Root: {selectedProject.rootPath}</p>
                {selectedRootInfo?.isGitRepo && (
                  <>
                    <span className="pill">Git repo detected</span>
                    <p className="muted">
                      This project is a Git repo; imports will be committed normally.
                    </p>
                  </>
                )}
              </div>
            )}
            {selectedProjectId && studies.length === 0 && (
              <p className="muted">No studies yet.</p>
            )}
            <ul className="list">
              {studies.map((study) => (
                <li key={study.id}>
                  <div className="list-row">
                    <button
                      className={`study-select ${
                        study.id === selectedStudyId ? "active" : ""
                      }`}
                      onClick={() => setSelectedStudyId(study.id)}
                    >
                      <strong>{study.title}</strong>
                      <span>{study.createdAt}</span>
                    </button>
                    <button
                      className="list-action"
                      onClick={(event) => {
                        event.stopPropagation();
                        if (selectedProjectId) {
                          openAnalysisModal(selectedProjectId, study.id);
                        }
                      }}
                      disabled={!selectedProjectId}
                    >
                      Add analysis
                    </button>
                    <button
                      className="list-action ghost"
                      onClick={(event) => {
                        event.stopPropagation();
                        if (selectedProjectId) {
                          openRemoveAnalysisModal(selectedProjectId, study.id);
                        }
                      }}
                      disabled={!selectedProjectId}
                    >
                      Remove analysis
                    </button>
                  </div>
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
                <div className="tabs">
                  <button
                    className={studyTab === "overview" ? "active" : ""}
                    onClick={() => setStudyTab("overview")}
                  >
                    Overview
                  </button>
                  <button
                    className={studyTab === "files" ? "active" : ""}
                    onClick={() => setStudyTab("files")}
                  >
                    Files
                  </button>
                  <button
                    className={studyTab === "danger" ? "active" : ""}
                    onClick={() => setStudyTab("danger")}
                  >
                    Danger Zone
                  </button>
                </div>

                {studyTab === "overview" && (
                  <>
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
                            onChange={(event) =>
                              setStudyTitleDraft(event.target.value)
                            }
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
                  </>
                )}

                {studyTab === "files" && (
                  <div className="files">
                    <div className="panel-header compact">
                      <h3>Imported Files</h3>
                      <button onClick={handleImportFiles}>Import files</button>
                    </div>
                    {(selectedStudy.files ?? []).length === 0 && (
                      <p className="muted">No files yet.</p>
                    )}
                    {(selectedStudy.files ?? []).length > 0 && (
                      <ul className="file-list">
                        {(selectedStudy.files ?? []).map((file) => (
                          <li key={file.path}>
                            <div className="file-row">
                              <div>
                                <strong>{file.name}</strong>
                                <div className="file-meta">
                                  <span className="file-kind">{file.kind}</span>
                                  <span>{file.path}</span>
                                </div>
                              </div>
                              {selectedProject && isImageKind(file.kind) && (
                                <img
                                  src={convertFileSrc(
                                    buildFilePath(selectedProject.rootPath, file.path)
                                  )}
                                  alt={file.name}
                                  className="file-preview"
                                />
                              )}
                            </div>
                            <button
                              className="ghost"
                              onClick={() => handleRemoveFile(file.path)}
                            >
                              Remove
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}

                {studyTab === "danger" && (
                  <div className="danger-zone">
                    <p className="muted">
                      Deleting a study removes it from the project. You can
                      optionally delete its folder on disk.
                    </p>
                    <label className="checkbox compact">
                      <input
                        type="checkbox"
                        checked={deleteStudyOnDisk}
                        onChange={(event) =>
                          setDeleteStudyOnDisk(event.target.checked)
                        }
                      />
                      Delete study folder on disk
                    </label>
                    <button className="danger" onClick={handleDeleteStudy}>
                      Delete Study
                    </button>
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

              <label>Project Folder *</label>
              <div className="inline-actions">
                <label>
                  <input
                    type="radio"
                    name="project-root-mode"
                    checked={projectRootMode === "existing"}
                    onChange={() => {
                      setProjectRootMode("existing");
                      setProjectRoot("");
                      setProjectRootInfo(null);
                    }}
                  />
                  Use existing folder
                </label>
                <label>
                  <input
                    type="radio"
                    name="project-root-mode"
                    checked={projectRootMode === "new"}
                    onChange={() => {
                      setProjectRootMode("new");
                      setProjectRoot("");
                      setProjectRootInfo(null);
                    }}
                  />
                  Create new folder
                </label>
              </div>

              <label>Project Root Location *</label>
              <div className="inline-field">
                <input
                  value={projectRoot}
                  placeholder={
                    projectRootMode === "existing"
                      ? "Choose existing project folder"
                      : "Choose parent folder"
                  }
                  readOnly
                />
                <button onClick={handlePickProjectRoot}>Choose</button>
              </div>
              {projectFormErrors.root && (
                <p className="field-error">{projectFormErrors.root}</p>
              )}
              {projectRootInfo?.isGitRepo && (
                <p className="muted">Git repo detected in selected folder.</p>
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

      {isAnalysisModalOpen && analysisTarget && (
        <AnalysisTemplateWizard
          isOpen={isAnalysisModalOpen}
          projectId={analysisTarget.projectId}
          studyId={analysisTarget.studyId}
          loading={loading}
          onClose={closeAnalysisModal}
          onSubmit={(options) =>
            handleAddAnalysis(
              analysisTarget.projectId,
              analysisTarget.studyId,
              options
            )
          }
        />
      )}

      {isRemoveAnalysisModalOpen && removeAnalysisTarget && (
        <div className="modal-backdrop" onClick={closeRemoveAnalysisModal}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h2>Remove Analysis</h2>
              <button className="ghost" onClick={closeRemoveAnalysisModal}>
                Close
              </button>
            </div>
            <div className="modal-body">
              {analysisFiles.length === 0 && (
                <p className="muted">No analysis files found.</p>
              )}
              {analysisFiles.length > 0 && (
                <ul className="list">
                  {analysisFiles.map((name) => (
                    <li key={name}>
                      <div className="list-row">
                        <div className="analysis-name">{name}.Rmd</div>
                        <button
                          className="danger"
                          onClick={() =>
                            handleRemoveAnalysis(
                              removeAnalysisTarget.projectId,
                              removeAnalysisTarget.studyId,
                              name
                            )
                          }
                        >
                          Delete
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      )}

      {isProjectSettingsOpen && selectedProject && (
        <div className="modal-backdrop" onClick={closeProjectSettings}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h2>Project Settings</h2>
              <button className="ghost" onClick={closeProjectSettings}>
                Close
              </button>
            </div>
            <div className="modal-body">
              <p className="muted">Project: {selectedProject.name}</p>
              <label>Root Folder</label>
              <div className="inline-field">
                <input value={projectRootEdit} readOnly />
                <button onClick={handlePickProjectRootEdit}>Choose</button>
              </div>
              {projectRootEditError && (
                <p className="field-error">{projectRootEditError}</p>
              )}
              {(projectRootEditInfo?.isGitRepo ||
                selectedRootInfo?.isGitRepo) && (
                <p className="muted">Git repo detected in selected folder.</p>
              )}
              <label>Delete Options</label>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={deleteProjectOnDisk}
                  onChange={(event) => setDeleteProjectOnDisk(event.target.checked)}
                />
                Also delete the project folder on disk
              </label>
            </div>
            <div className="modal-actions">
              <button className="ghost" onClick={closeProjectSettings}>
                Cancel
              </button>
              <button className="danger" onClick={handleDeleteProject}>
                Delete Project
              </button>
              <button onClick={handleUpdateProjectRoot}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
