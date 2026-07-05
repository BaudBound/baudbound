import type * as React from "react";

import { cn } from "@/lib/utils";

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
	return (
		<input
			type={type}
			data-slot="input"
			className={cn(
				"h-8 w-full min-w-0 rounded-lg border border-input bg-transparent px-2.5 py-1 text-base transition-[border-color,background-color,box-shadow] outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:border-baud-red/75 focus-visible:ring-0 focus-visible:shadow-[0_0_0_2px_rgb(230_45_62_/_0.14)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-input/50 disabled:opacity-50 aria-invalid:border-destructive/80 aria-invalid:ring-0 aria-invalid:shadow-[0_0_0_2px_rgb(224_92_92_/_0.14)] md:text-sm dark:bg-input/30 dark:disabled:bg-input/80 dark:aria-invalid:border-destructive/70",
				className,
			)}
			{...props}
		/>
	);
}

export { Input };
