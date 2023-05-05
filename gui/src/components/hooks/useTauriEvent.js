import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export default function useTauriEvent(event, callback) {
    useEffect(()=>{
        const unlisten = listen(event, callback);

        return () => {
            unlisten.then(f => f());
        }
    }, []);
}