import dynamic from 'next/dynamic';
import localFont from 'next/font/local';
import '../styles/global.scss';
import "material-symbols/outlined.css";

const Titlebar = dynamic(() => import('../components/Titlebar'), {
    ssr: false
});

const font = localFont({
    src: [
        {
            path: '../assets/fonts/GoogleSans-Regular.ttf',
            weight: '400',
            style: 'normal'
        },
        {
            path: '../assets/fonts/GoogleSans-Italic.ttf',
            weight: '400',
            style: 'italic'
        },
        {
            path: '../assets/fonts/GoogleSans-Medium.ttf',
            weight: '500',
            style: 'normal'
        },
        {
            path: '../assets/fonts/GoogleSans-MediumItalic.ttf',
            weight: '500',
            style: 'italic'
        },
        {
            path: '../assets/fonts/GoogleSans-Bold.ttf',
            weight: '700',
            style: 'normal'
        },
        {
            path: '../assets/fonts/GoogleSans-BoldItalic.ttf',
            weight: '700',
            style: 'italic'
        }
    ]
});

// This default export is required in a new `pages/_app.js` file.
export default function MyApp({ Component, pageProps }) {
    const getLayout = Component.getLayout || ((page) => page);
    return (
        <>
            <style jsx global>{`
                html {
                    font-family: ${font.style.fontFamily};
                    --font-family: ${font.style.fontFamily};
                }
            `}</style>
            <Titlebar />
            {getLayout(<Component {...pageProps} />)}
        </>
    );
}
