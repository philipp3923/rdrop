import { appWindow } from '@tauri-apps/api/window'

export default function Titlebar() {
    const handleMinimize = () => {
        appWindow.minimize();
    }

    const handleMaximize = () => {
        appWindow.maximize();
    }

    const handleClose = () => {
        appWindow.close();
    }

    return (
        <div data-tauri-drag-region className='titlebar'>
            <div className='titlebar-button' id='titlebar-minimize' onClick={handleMinimize}>
                <span>&#xE921;</span>
            </div>
            <div className='titlebar-button' id='titlebar-maximize' onClick={handleMaximize}>
                <span>&#xE922;</span>
            </div>
            <div className='titlebar-button' id='titlebar-close' onClick={handleClose}>
                <span>&#xE8BB;</span>
            </div>
        </div>
    );
}
