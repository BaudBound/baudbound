import "@/styles/globals.css";
import type { Metadata, Viewport } from "next";

const appName = "BaudBound Editor";
const appDescription =
	"Create local automation workflows visually and export them as portable BaudBound .bbs packages.";
const brandColor = "#e62d3e";

export const dynamic = "force-dynamic";

const appUrl =
	process.env.EDITOR_URL ??
	process.env.NEXT_PUBLIC_EDITOR_URL ??
	process.env.NEXT_PUBLIC_SITE_URL ??
	"https://editor.baudbound.app";

export const metadata: Metadata = {
	metadataBase: new URL(appUrl),
	applicationName: appName,
	title: {
		default: appName,
		template: `%s | ${appName}`,
	},
	description: appDescription,
	keywords: [
		"BaudBound",
		"BaudBound Editor",
		"visual scripting",
		"automation",
		"local-first automation",
		"script editor",
		".bbs",
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
		siteName: "BaudBound",
		title: appName,
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
		title: appName,
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
	return (
		<html lang="en" className="h-full antialiased">
			<body className="min-h-full flex flex-col">{children}</body>
		</html>
	);
}
