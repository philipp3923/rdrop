import { useEffect, useState } from 'react';
import MatIcon from './MatIcon';
import IconButton from './IconButton';
import useTauriEvent from './hooks/useTauriEvent';
import { emit } from '@tauri-apps/api/event';
import {filesize} from 'filesize';
import { invoke } from '@tauri-apps/api/tauri';
import { FileState } from '../vendor/file';
import { save } from '@tauri-apps/api/dialog';

export default function TransferList() {
    const [hover, setHover] = useState(false);
    const [files, setFiles] = useState([]);

    useEffect(() => {
    }, []);

    const mutateFile = (file) => {
        if (typeof file?.size !== 'string') {
            file.size = filesize(file.size, { standard: "jedec"});
        }
        return file;
    }

    useTauriEvent('tauri://file-drop', (event) => {
        console.log(event);

        if(event.payload.length > 0) {
            event.payload.forEach((file) => {
                invoke('offer_file', { path: file });
            });
        }
        
        setHover(false);
    });

    useTauriEvent('tauri://file-drop-hover', (event) => {
        setHover(true);
    });

    useTauriEvent('tauri://file-drop-cancelled', (event) => {
        setHover(false);
    });

    useTauriEvent('app://file-update', (event) => {
        const file = mutateFile(event.payload);
        let newFiles = files.map((f) => f.hash === file.hash ? file : f);
        if(!newFiles.some((f) => f.hash === file.hash))
            newFiles = [file, ...newFiles];
        setFiles(newFiles);
    });


    const handleCancel = (file) => {
        if(file.state === FileState.PENDING)
            invoke('deny_file', { hash: file.hash });
        else if(file.state === FileState.TRANSFERING)
            invoke('stop_file', { hash: file.hash });
    };

    const handleDownload = async (file) => {
        const filePath = await save({ defaultPath: file.name });
        console.log(filePath);
        invoke('accept_file', { hash: file.hash, path: filePath });
    };

    const handleShowInExplorer = async (file) => {
        invoke('show_in_folder', { path: file.path });
    };

    let classes = 'transfer-list container container-secondary';
    if (hover) classes += ' hover';

    return (
        <div className={classes}>
            <div className='transfer-list-overlay'>
                <MatIcon size='large'>file_upload</MatIcon>
                <h1 className='headline-large'>Drop files here</h1>
            </div>
            <div className='transfer-list-items'>
                {files.map((file) => {
                    const canDownload = file.state === 'pending' && !file.is_sender;
                    const canCancel = file.state !== 'completed' && !file.is_sender;
                    const canShowInExplorer = file.state === 'completed' && !file.is_sender;
                    return (
                        <div className={'transfer-list-item' + (file.is_sender ? " sender" : "")} key={file.id}>
                            <div className='transfer-list-item-icon'>
                                {file.is_sender && <MatIcon>vertical_align_top</MatIcon>}
                                {!file.is_sender && <MatIcon>vertical_align_bottom</MatIcon>}
                            </div>
                            <div className='transfer-list-item-content'>
                                <h1 className='title-medium'>{file.name}</h1>
                                <p className='body-large'>{file.size}</p>
                            </div>
                            <div className='transfer-list-item-status'>
                                {file.state === "pending" && <p className='body-large'>Pending</p>}
                                {file.state === "transfering" && <p className='body-large'>{file.status}%</p>}
                                {file.state === "completed" && <p className='body-large'>Completed</p>}
                            </div>
                            <div className='transfer-list-item-actions flex'>
                                {canDownload && <IconButton text onClick={()=>handleDownload(file)}>download</IconButton>}
                                {canCancel && <IconButton text onClick={()=>handleCancel(file)}>close</IconButton>}
                                {canShowInExplorer && <IconButton text onClick={()=>handleShowInExplorer(file)}>folder</IconButton>}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
