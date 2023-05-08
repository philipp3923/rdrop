import { useEffect, useState } from 'react';
import MatIcon from './MatIcon';
import IconButton from './IconButton';
import useTauriEvent from './hooks/useTauriEvent';
import { emit } from '@tauri-apps/api/event';
import {filesize} from 'filesize';

export default function TransferList() {
    const [hover, setHover] = useState(false);
    const [files, setFiles] = useState([]);

    useEffect(() => {
        let files = [{
            id: 1,
            name: 'test.txt',
            size: 84684438,
            status: 89,
            state: 'pending',
            is_sender: false,
        },
        {
            id: 1,
            name: 'test.txt',
            size: 84684438,
            status: 89,
            state: 'transfering',
            is_sender: false,
        },
        {
            id: 2,
            name: 'RAF CAMORA.mp3',
            size: 4864846843,
            status: 72,
            state: 'completed',
            is_sender: true,
        }];

        files = [...files, ...files, ...files, ...files, ...files];

        setFiles(files.map(mutateFile));
    }, []);

    const mutateFile = (file) => {
        if (typeof file?.size !== 'string') {
            file.size = filesize(file.size, { standard: "jedec"});
        }
        return file;
    }

    useTauriEvent('tauri://file-drop', (event) => {
        console.log(event);

        emit('app://add-file', event.payload);
        setHover(false);
    });

    useTauriEvent('tauri://file-drop-hover', (event) => {
        console.log(event);
        setHover(true);
    });

    useTauriEvent('tauri://file-drop-cancelled', (event) => {
        console.log(event);
        setHover(false);
    });

    useTauriEvent('app://file-added', (event) => {
        const file = mutateFile(event.payload);
        setFiles([...files, file]);
    });

    useTauriEvent('app://file-updated', (event) => {
        const file = mutateFile(event.payload);
        setFiles([...files.filter((f) => f.id !== file.id), file]);
    });

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
                                {canDownload && <IconButton text>download</IconButton>}
                                {canCancel && <IconButton text>close</IconButton>}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
