import { appWindow } from '@tauri-apps/api/window'
import MatIcon from './MatIcon';

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
                <MatIcon size="small">minimize</MatIcon>
            </div>
            <div className='titlebar-button' id='titlebar-maximize' onClick={handleMaximize}>
                <MatIcon size="small">stop</MatIcon>
            </div>
            <div className='titlebar-button' id='titlebar-close' onClick={handleClose}>
                <MatIcon size="small">close</MatIcon>
            </div>
        </div>
    );
}
