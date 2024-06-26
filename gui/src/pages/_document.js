import { Html, Head, Main, NextScript } from "next/document";

export default function Document() {
    return (
        <Html className="theme theme-light">
            <Head>
                <link rel='apple-touch-icon' sizes='180x180' href='/icons/apple-touch-icon.png' />
                <link rel='shortcut icon' href='/favicon.ico' />
                <link rel="manifest" href="/site.webmanifest" />
                <link rel="mask-icon" href="/safari-pinned-tab.svg" color="#5bbad5" />
                <meta name="msapplication-TileColor" content="#da532c" />
                <meta name="theme-color" content="#ffffff" />
                <meta name="mobile-web-app-capable" content="yes" />
                <meta name="apple-mobile-web-app-capable" content="yes" />
            </Head>
            <body>
                <Main />
                <NextScript />
            </body>
        </Html>
    );
}
