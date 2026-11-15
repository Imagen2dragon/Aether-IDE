import React, { useEffect } from "react";

export interface NotificationItem {
  id: string;
  title: string;
  message: string;
}

interface NotificationsProps {
  notifications: NotificationItem[];
  onDismiss: (id: string) => void;
}

export const Notifications: React.FC<NotificationsProps> = ({ notifications, onDismiss }) => {
  return (
    <div
      style={{
        position: "fixed",
        bottom: 30,
        right: 16,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        zIndex: 99999,
        pointerEvents: "none",
      }}
    >
      {notifications.map((n) => (
        <NotificationToast key={n.id} notification={n} onDismiss={onDismiss} />
      ))}
    </div>
  );
};

const NotificationToast: React.FC<{
  notification: NotificationItem;
  onDismiss: (id: string) => void;
}> = ({ notification, onDismiss }) => {
  useEffect(() => {
    const timer = setTimeout(() => {
      onDismiss(notification.id);
    }, 4000);
    return () => clearTimeout(timer);
  }, [notification.id, onDismiss]);

  return (
    <div
      style={{
        width: 360,
        backgroundColor: "#252526",
        border: "1px solid var(--vscode-border-light)",
        borderLeft: "3px solid var(--vscode-accent)",
        boxShadow: "0 4px 14px rgba(0, 0, 0, 0.45)",
        padding: "10px 14px",
        display: "flex",
        flexDirection: "column",
        gap: 4,
        pointerEvents: "all",
        borderRadius: 2,
        userSelect: "none",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "#ffffff" }}>
          {notification.title}
        </span>
        <button
          onClick={() => onDismiss(notification.id)}
          style={{ color: "var(--vscode-text-dim)", fontSize: 12 }}
          title="Dismiss"
        >
          ✕
        </button>
      </div>

      <div style={{ fontSize: 11, color: "var(--vscode-text-dim)", lineHeight: 1.4 }}>
        {notification.message}
      </div>
    </div>
  );
};
