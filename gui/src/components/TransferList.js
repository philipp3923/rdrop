import { useState } from "react";
import MatIcon from "./MatIcon";
import useTauriEvent from "./hooks/useTauriEvent";
import { emit } from "@tauri-apps/api/event";

export default function TransferList() {
    const [hover, setHover] = useState(false);

    useTauriEvent("tauri://file-drop", (event) => {
        console.log(event);

        emit("app://add-file", event.payload);
        setHover(false);
    })

    useTauriEvent("tauri://file-drop-hover", (event) => {
        console.log(event);
        setHover(true);
    })

    useTauriEvent("tauri://file-drop-cancelled", (event) => {
        console.log(event);
        setHover(false);
    })

    let classes = "transfer-list container container-secondary";
    if(hover) classes += " hover";

    return <div className={classes}>
        <div className="transfer-list-overlay">
            <MatIcon size="large">file_upload</MatIcon>
            <h1 className="headline-large">Drop files here</h1>
        </div>
        <div className="transfer-list-items" />
    </div>

}