export const getPublicIPv6 = async () => {
    const res = await fetch('https://api64.ipify.org?format=json');
    const data = await res.json();
    return data.ip;
};

export const getPublicIPv4 = async () => {
    const res = await fetch('https://api.ipify.org?format=json');
    const data = await res.json();
    return data.ip;
};

export const getPublicIP = async () => {
    const ipv4 = await getPublicIPv4();
    const ipv6 = await getPublicIPv6();
    return { ipv4, ipv6 };
};