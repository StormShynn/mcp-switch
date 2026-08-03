import type { AppId, McpServerEntry } from "../lib/types";
import { APPS, APP_COLORS } from "../lib/types";

export function TrashRow({
  server,
  index,
  onRestore,
  onDeleteForever,
}: {
  server: McpServerEntry;
  index: number;
  onRestore: (serverName: string, appId: AppId) => void;
  onDeleteForever: (serverName: string, appId: AppId) => void;
}) {
  const appLabel = APPS.find((a) => a.id === server.app)?.label ?? server.app;

  return (
    <div className="server-row fade-in" style={{ animationDelay: `${index * 30}ms` }}>
      <div className="server-info">
        <div className="server-name">{server.name}</div>
        <div className="server-command">
          {server.transport === "stdio" ? server.command : server.url}
          <span className="badge" style={{ color: APP_COLORS[server.app] }}>
            {appLabel}
          </span>
          <span className="badge badge-trash">No longer found</span>
        </div>
      </div>

      <div className="server-toggles">
        <button className="btn btn-sm" onClick={() => onRestore(server.name, server.app)}>
          Restore
        </button>
        <button
          className="btn btn-sm btn-danger"
          onClick={() => onDeleteForever(server.name, server.app)}
        >
          Delete forever
        </button>
      </div>
    </div>
  );
}
