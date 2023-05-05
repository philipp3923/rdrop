import Button from '../components/Button';
import Layout from '../layouts/Layout';
import MatIcon from '../components/MatIcon';
import { useRouter } from 'next/router';
import TransferList from '../components/TransferList';

export default function Transfer() {
    const router = useRouter();

    const handleClose = () => {
        router.push('/');
    }

    return (
        <div className='transfer'>
            <section className='layout-large m-t-24'>
                <h1 className='headline-large m-b-24'>rdrop.</h1>
            </section>
            <section className='layout-large transfer-main p-b-24'>
                <div className="transfer-actions">
                    <Button tonal large>
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
