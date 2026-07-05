import type * as React from "react";

import { cn } from "@/lib/utils";

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
	return (
		<textarea
			data-slot="textarea"
			className={cn(
				"flex field-sizing-content min-h-16 w-full rounded-lg border border-input bg-transparent px-2.5 py-2 text-base transition-[border-color,background-color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-baud-red/75 focus-visible:ring-0 focus-visible:shadow-[0_0_0_2px_rgb(230_45_62_/_0.14)] disabled:cursor-not-allowed disabled:bg-input/50 disabled:opacity-50 aria-invalid:border-destructive/80 aria-invalid:ring-0 aria-invalid:shadow-[0_0_0_2px_rgb(224_92_92_/_0.14)] md:text-sm dark:bg-input/30 dark:disabled:bg-input/80 dark:aria-invalid:border-destructive/70",
				className,
			)}
			{...props}
		/>
	);
}

export { Textarea };
