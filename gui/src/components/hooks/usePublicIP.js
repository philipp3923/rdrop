import { useEffect, useState } from "react";
import { getPublicIP } from "../../vendor/ip";

let cache = { ipv4: null, ipv6: null};

export const usePublicIP = () => {
    const [ip, setIp] = useState({ ipv4: cache.ipv4 || "LOADING", ipv6: cache.ipv6 || "LOADING" });

    useEffect(() => {
        getPublicIP().then((data) => {
            if(data.ipv6.includes(".")) data.ipv6 = "NOT AVAILABLE";
            setIp(data);
            cache = data;
        });
    }, []);

    return ip;
}