import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import Button from '../components/Button';
import InputField from '../components/InputField';
import Layout from '../layouts/Layout';
import MatIcon from '../components/MatIcon';
import { usePublicIP } from '../components/hooks/usePublicIP';
import Link from 'next/link';
import { useRouter } from 'next/router';
import Loader from '../components/Loader';
import useTauriEvent from '../components/hooks/useTauriEvent';
import { emit } from '@tauri-apps/api/event';
import { isValidIP, isValidPort } from '../vendor/ip';

export default function Home() {
    const router = useRouter();
    const [isConnecting, setConnecting] = useState('none');
    const [connectionStatus, setConncectionStatus] = useState({ status: '' });
    const [ipv6Port, setIPv6Port] = useState("187");
    const [ipError, setIPError] = useState(false);
    const [portError, setPortError] = useState(false);
    const ipRef = useRef(null);
    const portRef = useRef(null);

    const ip = usePublicIP();

    async function handleConnect() {
        const ip = ipRef.current.value;
        const port = portRef.current.value;

        if (!isValidIP(ip)) {
            setIPError('Address is invalid');
            return;
        }
        setIPError(false);

        if (!isValidPort(port)) {
            setPortError('Port is invalid');
            return;
        }
        setPortError(false);
        setConnecting('connecting');

        emit('app://connect', { ip, port });
    }

    async function handleAbort() {
        setConnecting('none');
    }

    useTauriEvent('app://update-status', (event) => {
        setConncectionStatus(event?.payload);
    });

    useTauriEvent('app://update-port', (event) => {
        setIPv6Port(event?.payload);
    });

    useTauriEvent('app://socket-failed', (event) => {
        setConnecting('failed');
        setConncectionStatus(event?.payload);
    });

    useTauriEvent('app://connected', () => {
        router.push('/transfer');
    });

    return (
        <div className='home layout-center-height'>
            <section className='layout-large'>
                <h1 className='display-large m-b-24'>rdrop.</h1>
            </section>
            <section className='layout-large'>
                <div className={'home-container container container-secondary connection-' + isConnecting}>
                    <div className='home-ip'>
                        <h2 className='title-medium'>IPv4 Address</h2>
                        <h1 className='headline-large'>{ip.ipv4}</h1>
                        <h2 className='title-medium m-t-16'>IPv6 Address</h2>
                        <h1 className='headline-large'>{ip.ipv6}<span>:{ipv6Port}</span></h1>
                    </div>
                    <div className={'home-status' + (connectionStatus.error ? ' error' : '')}>
                        <Loader />
                        <div className="m-l-24">
                            <h2 className='headline-large'>{connectionStatus.status}</h2>
                            {connectionStatus.description && (
                                <p className='title-medium'>{connectionStatus.description}</p>
                            )}
                        </div>
                    </div>
                </div>
            </section>
            <section className='layout-large m-t-24'>
                <form
                    onSubmit={(e) => {
                        e.preventDefault();
                        handleConnect();
                    }}
                    className='connect-form'
                >
                    <InputField id='ip' label='Partner IP Address' ref={ipRef} error={ipError} />
                    <InputField
                        type='number'
                        id='port'
                        label='Port'
                        ref={portRef}
                        error={portError}
                        defaultValue='2000'
                    />
                    {isConnecting === 'connecting' && (
                        <Button tonal large onClick={handleAbort}>
                            Cancel
                            <MatIcon right>close</MatIcon>
                        </Button>
                    )}
                    {isConnecting !== 'connecting' && (
                        <Button type='submit' tonal large>
                            Connect
                            <MatIcon right>arrow_forward</MatIcon>
                        </Button>
                    )}
                </form>
            </section>
        </div>
    );
}

Home.getLayout = (page) => {
    return <Layout>{page}</Layout>;
};
