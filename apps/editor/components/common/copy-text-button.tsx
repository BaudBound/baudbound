import { CheckCircle2, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";

type CopyTextButtonProps = {
	label: string;
	text: string;
};

export function CopyTextButton({ text, label }: CopyTextButtonProps) {
	const timeoutRef = useRef<number | null>(null);
	const [copied, setCopied] = useState(false);

	useEffect(() => {
		return () => {
			if (timeoutRef.current !== null) {
				window.clearTimeout(timeoutRef.current);
			}
		};
	}, []);

	const handleCopy = () => {
		if (!navigator.clipboard) {
			return;
		}

		void navigator.clipboard.writeText(text).then(() => {
			if (timeoutRef.current !== null) {
				window.clearTimeout(timeoutRef.current);
			}
			setCopied(true);
			timeoutRef.current = window.setTimeout(() => {
				timeoutRef.current = null;
				setCopied(false);
			}, 1200);
		});
	};

	return (
		<Button
			type="button"
			onClick={handleCopy}
			className="size-6 shrink-0"
			aria-label={label}
			title={label}
			size="xsIcon"
			variant="ghost"
		>
			{copied ? <CheckCircle2 size={13} className="text-baud-green" /> : <Copy size={13} />}
		</Button>
	);
}
