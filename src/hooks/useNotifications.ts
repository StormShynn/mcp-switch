import { useCallback, useState } from "react";

export function useNotifications() {
  const [notification, setNotification] = useState<{
    message: string;
    type: "success" | "error";
  } | null>(null);

  const notify = useCallback((message: string, type: "success" | "error") => {
    setNotification({ message, type });
    setTimeout(() => setNotification(null), 3000);
  }, []);

  return { notification, notify };
}
