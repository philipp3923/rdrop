import Button from '../components/Button';
import Layout from '../layouts/Layout';
import MatIcon from '../components/MatIcon';
import { useRouter } from 'next/router';
import TransferList from '../components/TransferList';
import { open } from '@tauri-apps/api/dialog';
import { invoke } from '@tauri-apps/api/tauri';
import useTauriEvent from '../components/hooks/useTauriEvent';

export default function Transfer() {
    const router = useRouter();

    useTauriEvent("app://disconnected", () => {
        router.push('/');
    });

    const handleClose = () => {
        router.push('/');
    };

    const handleUpload = async () => {
        const selected = await open({
            multiple: true
        });

        if(!selected) return;
        selected.forEach((file) => {
            invoke('offer_file', { path: file });
        });
    };

    return (
        <div className='transfer'>
            <section className='layout-large m-t-24'>
                <h1 className='headline-large m-b-24'>rdrop.</h1>
            </section>
            <section className='layout-large transfer-main p-b-24'>
                <div className='transfer-actions'>
                    <Button tonal large onClick={handleUpload}>
                        <MatIcon left>file_upload</MatIcon>
                        Upload File
                    </Button>
                    <Button text onClick={handleClose}>
                        <MatIcon left>close</MatIcon>
                        Close
                    </Button>
                </div>
                <div className='transfer-files'>
                    <TransferList />
                </div>
            </section>
        </div>
    );
}

Transfer.getLayout = (page) => {
    return <Layout>{page}</Layout>;
};
