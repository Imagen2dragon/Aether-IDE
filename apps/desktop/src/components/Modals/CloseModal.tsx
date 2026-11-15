import React from "react";

interface CloseModalProps {
  isOpen: boolean;
  filename: string;
  onSave: () => void;
  onDontSave: () => void;
  onCancel: () => void;
}

export const CloseModal: React.FC<CloseModalProps> = ({
  isOpen,
  filename,
  onSave,
  onDontSave,
  onCancel,
}) => {
  if (!isOpen) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.55)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 999999,
      }}
      onClick={onCancel}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 440,
          backgroundColor: "#252526",
          border: "1px solid var(--vscode-border-light)",
          boxShadow: "0 8px 24px rgba(0, 0, 0, 0.6)",
          padding: 20,
          borderRadius: 4,
          userSelect: "none",
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 600, color: "#ffffff", marginBottom: 8 }}>
          Save changes?
        </div>
        <p style={{ fontSize: 12, color: "var(--vscode-text-dim)", marginBottom: 20, lineHeight: 1.5 }}>
          Do you want to save the changes you made to <strong>{filename}</strong>? Your changes will be lost if you don't save them.
        </p>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            onClick={onSave}
            className="vscode-btn-primary"
            style={{ minWidth: 70 }}
          >
            Save
          </button>
          <button
            onClick={onDontSave}
            className="vscode-btn-secondary"
            style={{ minWidth: 80 }}
          >
            Don't Save
          </button>
          <button
            onClick={onCancel}
            className="vscode-btn-secondary"
            style={{ minWidth: 70 }}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};
