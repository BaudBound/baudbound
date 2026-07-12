import type { Metadata, Viewport } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import Script from "next/script";

import "../styles/globals.css";

// Read fresh per request so compose.yaml env vars take effect without rebuilding the image.
export const dynamic = "force-dynamic";

const appName = "BaudBound";
const appDescription =
	"Build visual automation workflows in your browser and run them natively on your own Windows or Linux machine. No cloud account required.";
const brandColor = "#e62d3e";
const siteUrl = process.env.WEBSITE_URL ?? process.env.NEXT_PUBLIC_SITE_URL ?? "https://baudbound.app";

const inter = Inter({
	subsets: ["latin"],
	variable: "--font-inter",
	weight: ["400", "500", "600", "700", "800"],
});

const jetbrainsMono = JetBrains_Mono({
	subsets: ["latin"],
	variable: "--font-jetbrains-mono",
	weight: ["400", "500", "600", "700"],
});

export const metadata: Metadata = {
	metadataBase: new URL(siteUrl),
	applicationName: appName,
	title: {
		default: "BaudBound: Automate your machine, visually.",
		template: `%s | ${appName}`,
	},
	description: appDescription,
	keywords: [
		"BaudBound",
		"visual automation",
		"local-first automation",
		"workflow editor",
		"desktop automation",
		"Windows automation",
		"Linux automation",
	],
	authors: [{ name: "NATroutter" }],
	creator: "NATroutter",
	publisher: "NATroutter",
	category: "technology",
	referrer: "origin-when-cross-origin",
	alternates: {
		canonical: "/",
	},
	icons: {
		icon: [
			{ url: "/icon_x16.ico", sizes: "16x16", type: "image/x-icon" },
			{ url: "/icon_x32.ico", sizes: "32x32", type: "image/x-icon" },
			{ url: "/icon_x48.ico", sizes: "48x48", type: "image/x-icon" },
			{ url: "/icon_x64.ico", sizes: "64x64", type: "image/x-icon" },
			{ url: "/icon_x128.ico", sizes: "128x128", type: "image/x-icon" },
			{ url: "/icon_x256.ico", sizes: "256x256", type: "image/x-icon" },
		],
		shortcut: [{ url: "/icon_x32.ico" }],
		apple: [{ url: "/logo-notext.png", sizes: "800x800", type: "image/png" }],
	},
	openGraph: {
		type: "website",
		locale: "en_US",
		url: "/",
		siteName: appName,
		title: "BaudBound: Automate your machine, visually.",
		description: appDescription,
		images: [
			{
				url: "/logo.png",
				width: 800,
				height: 800,
				alt: "BaudBound logo",
				type: "image/png",
			},
		],
	},
	twitter: {
		card: "summary",
		title: "BaudBound: Automate your machine, visually.",
		description: appDescription,
		images: [{ url: "/logo.png", alt: "BaudBound logo" }],
	},
	robots: {
		index: true,
		follow: true,
		googleBot: {
			index: true,
			follow: true,
			"max-image-preview": "large",
			"max-snippet": -1,
		},
	},
	other: {
		"msapplication-TileColor": brandColor,
	},
};

export const viewport: Viewport = {
	width: "device-width",
	initialScale: 1,
	colorScheme: "dark",
	themeColor: brandColor,
};

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	const umamiTrackerSrc = process.env.UMAMI_TRACKER_SRC;
	const umamiRecorderSrc = process.env.UMAMI_RECORDER_SRC;
	const umamiWebsiteId = process.env.UMAMI_WEBSITE_ID;

	return (
		<html lang="en" className={`${inter.variable} ${jetbrainsMono.variable}`}>
			<body className="font-sans antialiased">
				{children}
				{umamiTrackerSrc && umamiWebsiteId && (
					<Script src={umamiTrackerSrc} data-website-id={umamiWebsiteId} strategy="afterInteractive" />
				)}
				{umamiTrackerSrc && umamiWebsiteId && umamiRecorderSrc && umamiRecorderSrc !== umamiTrackerSrc && (
					<Script src={umamiRecorderSrc} data-website-id={umamiWebsiteId} strategy="afterInteractive" />
				)}
			</body>
		</html>
	);
}
