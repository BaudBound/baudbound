"use client";

import { Switch as SwitchPrimitive } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

function Switch({
	className,
	size = "default",
	...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
	size?: "sm" | "default";
}) {
	return (
		<SwitchPrimitive.Root
			data-slot="switch"
			data-size={size}
			className={cn(
				"peer group/switch relative inline-flex shrink-0 cursor-pointer items-center rounded-full border border-baud-border bg-baud-border transition-[background-color,border-color,box-shadow] outline-none focus-visible:border-baud-red/75 focus-visible:shadow-[0_0_0_2px_rgb(230_45_62_/_0.14)] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-6 data-[size=default]:w-11 data-[size=sm]:h-5 data-[size=sm]:w-9 data-[state=checked]:border-baud-red data-[state=checked]:bg-baud-red data-[state=unchecked]:hover:border-baud-line data-[state=unchecked]:hover:bg-baud-line aria-invalid:border-destructive/80 aria-invalid:shadow-[0_0_0_2px_rgb(224_92_92_/_0.14)]",
				className,
			)}
			{...props}
		>
			<SwitchPrimitive.Thumb
				data-slot="switch-thumb"
				className={cn(
					"pointer-events-none block rounded-full bg-baud-text shadow-sm ring-0 transition-transform data-[state=checked]:bg-white data-[state=unchecked]:translate-x-0.5",
					size === "sm" ? "size-4 data-[state=checked]:translate-x-4" : "size-5 data-[state=checked]:translate-x-5",
				)}
			/>
		</SwitchPrimitive.Root>
	);
}

export { Switch };
