"use client";

import { type KeyboardEvent, useEffect, useId, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import { validateVariableName } from "@/data/project/variables";
import { cn } from "@/lib/utils";
import type { VariableCompletion } from "./variable-code-input";

type VariableNameInputProps = {
	hasError?: boolean;
	label: string;
	onChange: (value: string) => void;
	value: string;
	variables: VariableCompletion[];
};

export function VariableNameInput({ hasError, label, onChange, value, variables }: VariableNameInputProps) {
	const inputId = useId();
	const listId = useId();
	const [activeIndex, setActiveIndex] = useState(0);
	const [isOpen, setIsOpen] = useState(false);
	const suggestions = useMemo(() => getWritableVariableSuggestions(variables, value), [value, variables]);
	const showSuggestions = isOpen && suggestions.length > 0;

	useEffect(() => {
		setActiveIndex(0);
	}, [value]);

	const applySuggestion = (suggestion: VariableCompletion) => {
		onChange(suggestion.name);
		setIsOpen(false);
	};

	const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (!showSuggestions) {
			return;
		}

		if (event.key === "ArrowDown") {
			event.preventDefault();
			setActiveIndex((current) => (current + 1) % suggestions.length);
			return;
		}

		if (event.key === "ArrowUp") {
			event.preventDefault();
			setActiveIndex((current) => (current - 1 + suggestions.length) % suggestions.length);
			return;
		}

		if (event.key === "Enter" || event.key === "Tab") {
			const suggestion = suggestions[activeIndex];
			if (suggestion) {
				event.preventDefault();
				applySuggestion(suggestion);
			}
			return;
		}

		if (event.key === "Escape") {
			event.preventDefault();
			setIsOpen(false);
		}
	};

	return (
		<div className="relative">
			<label htmlFor={inputId} className="mb-1 block font-mono text-sm text-baud-muted">
				{label}
			</label>
			<Input
				id={inputId}
				value={value}
				autoComplete="off"
				aria-autocomplete="list"
				aria-controls={showSuggestions ? listId : undefined}
				aria-expanded={showSuggestions}
				aria-activedescendant={showSuggestions ? `${listId}-${activeIndex}` : undefined}
				role="combobox"
				onBlur={() => setIsOpen(false)}
				onFocus={() => setIsOpen(true)}
				onKeyDown={handleKeyDown}
				onChange={(event) => {
					onChange(event.target.value);
					setIsOpen(true);
				}}
				className={
					hasError
						? "border-baud-danger focus-visible:border-baud-danger"
						: "border-baud-border focus-visible:border-baud-red/75"
				}
			/>
			{showSuggestions && (
				<div
					id={listId}
					role="listbox"
					aria-label="Writable variables"
					className="absolute z-50 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-baud-border bg-baud-panel p-1 shadow-[0_16px_38px_rgba(0,0,0,0.42)]"
				>
					{suggestions.map((suggestion, index) => (
						<button
							id={`${listId}-${index}`}
							key={suggestion.name}
							type="button"
							role="option"
							aria-selected={index === activeIndex}
							className={cn(
								"flex w-full min-w-0 items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left hover:bg-baud-soft",
								index === activeIndex && "bg-baud-soft",
							)}
							onMouseEnter={() => setActiveIndex(index)}
							onMouseDown={(event) => {
								event.preventDefault();
								applySuggestion(suggestion);
							}}
						>
							<span className="min-w-0">
								<span className="block truncate font-mono text-sm text-baud-text">{suggestion.name}</span>
								{suggestion.description && (
									<span className="block truncate text-xs text-baud-muted">{suggestion.description}</span>
								)}
							</span>
							<span className="shrink-0 font-mono text-xs text-baud-muted">{suggestion.type}</span>
						</button>
					))}
				</div>
			)}
		</div>
	);
}

export function getWritableVariableSuggestions(variables: VariableCompletion[], query: string) {
	const normalizedQuery = query.trim().toLowerCase();

	return variables
		.filter((variable) => !variable.readOnly && !validateVariableName(variable.name))
		.filter((variable) => variable.name.toLowerCase() !== normalizedQuery)
		.filter((variable) => !normalizedQuery || variable.name.toLowerCase().includes(normalizedQuery))
		.sort((left, right) => {
			const leftStartsWithQuery = left.name.toLowerCase().startsWith(normalizedQuery);
			const rightStartsWithQuery = right.name.toLowerCase().startsWith(normalizedQuery);
			return Number(rightStartsWithQuery) - Number(leftStartsWithQuery) || left.name.localeCompare(right.name);
		})
		.slice(0, 12);
}
