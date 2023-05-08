import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import Button from "../components/Button";
import InputField from "../components/InputField";
import Layout from "../layouts/Layout";
import MatIcon from "../components/MatIcon";
import { usePublicIP } from "../components/hooks/usePublicIP";
import Link from "next/link";
import { useRouter } from "next/router";
import Loader from "../components/Loader";
import useTauriEvent from "../components/hooks/useTauriEvent";
import { emit } from "@tauri-apps/api/event";

export default function Home() {
    const router = useRouter();
    const [isConnecting, setConnecting] = useState("none");
    const [connectionStatus, setConncectionStatus] = useState("Connecting");
    const [ipError, setIPError] = useState(false);
    const ipRef = useRef(null);
    const ip = usePublicIP();

    async function handleConnect() {
        if (
            !ipRef.current.value.match(
                /((^\s*((([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.){3}([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5]))\s*$)|(^\s*((([0-9A-Fa-f]{1,4}:){7}([0-9A-Fa-f]{1,4}|:))|(([0-9A-Fa-f]{1,4}:){6}(:[0-9A-Fa-f]{1,4}|((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9A-Fa-f]{1,4}:){5}(((:[0-9A-Fa-f]{1,4}){1,2})|:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9A-Fa-f]{1,4}:){4}(((:[0-9A-Fa-f]{1,4}){1,3})|((:[0-9A-Fa-f]{1,4})?:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9A-Fa-f]{1,4}:){3}(((:[0-9A-Fa-f]{1,4}){1,4})|((:[0-9A-Fa-f]{1,4}){0,2}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9A-Fa-f]{1,4}:){2}(((:[0-9A-Fa-f]{1,4}){1,5})|((:[0-9A-Fa-f]{1,4}){0,3}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9A-Fa-f]{1,4}:){1}(((:[0-9A-Fa-f]{1,4}){1,6})|((:[0-9A-Fa-f]{1,4}){0,4}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(:(((:[0-9A-Fa-f]{1,4}){1,7})|((:[0-9A-Fa-f]{1,4}){0,5}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:)))(%.+)?\s*$))/
            )
        ) {
            setIPError("Address is invalid");
            return;
        }

        setConnecting("connecting");

        emit("app://start");

        setTimeout(() => {
            // router.push("/transfer")
        }, 5000);
    }

    async function handleAbort() {
        setConnecting("none");
    }

    useTauriEvent("app://update-status", (status) => {
        setConncectionStatus(status?.payload);
    });

    return (
        <div className="home layout-center-height">
            <section className="layout-large">
                <h1 className="display-large m-b-24">rdrop.</h1>
            </section>
            <section className="layout-large">
                <div className={"home-container container container-secondary connection-" + isConnecting}>
                    <div className="home-ip">
                        <h2 className="title-medium">IPv4 Address</h2>
                        <h1 className="headline-large">{ip.ipv4}</h1>
                        <h2 className="title-medium m-t-16">IPv6 Address</h2>
                        <h1 className="headline-large">{ip.ipv6}</h1>
                    </div>
                    <div className="home-status">
                        <Loader />
                        <h2 className="m-l-8 headline-large">{connectionStatus}</h2>
                    </div>
                </div>
            </section>
            <section className="layout-large m-t-24">
                <form
                    onSubmit={(e) => {
                        e.preventDefault();
                        handleConnect();
                    }}
                    className="connect-form"
                >
                    <InputField id="ip" label="Partner IP Address" ref={ipRef} error={ipError} />
                    <InputField id="port" onChange={(e) => setName(e.currentTarget.value)} label="Port" />
                    {isConnecting === "connecting" && (
                        <Button tonal large onClick={handleAbort}>
                            Cancel
                            <MatIcon right>close</MatIcon>
                        </Button>
                    )}
                    {isConnecting !== "connecting" && (
                        <Button type="submit" tonal large>
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
