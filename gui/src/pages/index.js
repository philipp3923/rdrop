import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import Button from '../components/Button';
import InputField from '../components/InputField';
import Layout from '../layouts/Layout';
import MatIcon from '../components/MatIcon';
import { usePublicIP } from '../components/hooks/usePublicIP';
import Link from 'next/link';

export default function Home() {
    const [greetMsg, setGreetMsg] = useState('');
    const [name, setName] = useState('');
    const ip = usePublicIP();

    async function greet() {
        // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
        setGreetMsg(await invoke('greet', { name }));
    }

    return (
        <div className='home layout-center-height'>
            <section className='layout-large'>
                <h1 className='display-large m-b-24'>rdrop.</h1>
            </section>
            <section className='layout-large'>
                <div className='container container-secondary'>
                    <h2 className='title-medium'>IPv4 Address</h2>
                    <h1 className='headline-large'>{ip.ipv4}</h1>
                    <h2 className='title-medium m-t-16'>IPv6 Address</h2>
                    <h1 className='headline-large'>{ip.ipv6}</h1>
                </div>
            </section>
            <section className='layout-large m-t-24'>
                <form
                    onSubmit={(e) => {
                        e.preventDefault();
                        greet();
                    }}
                    className='connect-form'
                >
                    <InputField
                        id='greet-input'
                        onChange={(e) => setName(e.currentTarget.value)}
                        label='Partner IP Address'
                    />
                    <InputField id='port' onChange={(e) => setName(e.currentTarget.value)} label='Port' />
                    <Link href='/transfer'>
                        <Button type='submit' tonal large>
                            Connect
                            <MatIcon right>arrow_forward</MatIcon>
                        </Button>
                    </Link>
                </form>

                <p className='body-large m-t-16'>{greetMsg}</p>
            </section>
        </div>
    );
}

Home.getLayout = (page) => {
    return <Layout>{page}</Layout>;
};
