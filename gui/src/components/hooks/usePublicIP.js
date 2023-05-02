import { useEffect, useState } from "react";
import { getPublicIP } from "../../vendor/ip";

export const usePublicIP = () => {
    const [ip, setIp] = useState({ ipv4: "LOADING", ipv6: "LOADING" });

    useEffect(() => {
        getPublicIP().then((data) => {
            setIp(data);
        });
    }, []);

    return ip;
}