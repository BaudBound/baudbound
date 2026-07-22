"use client";

import { type KeyboardEvent, type ReactNode, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";

export type VariableCompletion = {
	description?: string;
	name: string;
	readOnly: boolean;
	token: string;
	type: string;
};

type VariableCodeInputProps = {
	ariaLabel?: string;
	className?: string;
	containerClassName?: string;
	hasError?: boolean;
	id?: string;
	multiline?: boolean;
	onChange: (value: string) => void;
	placeholder?: string;
	value: string;
	variables: VariableCompletion[];
};

type CompletionState = {
	end: number;
	query: string;
	start: number;
};

export function VariableCodeInput({
	ariaLabel,
	className,
	containerClassName,
	hasError,
	id,
	multiline = false,
	onChange,
	placeholder,
	value,
	variables,
}: VariableCodeInputProps) {
	const textareaRef = useRef<HTMLTextAreaElement>(null);
	const highlightRef = useRef<HTMLPreElement>(null);
	const lineNumberRef = useRef<HTMLDivElement>(null);
	const [caretPosition, setCaretPosition] = useState(0);
	const [isFocused, setIsFocused] = useState(false);
	const lineCount = Math.max(1, value.split("\n").length);
	const lineNumberWidth = `calc(${String(lineCount).length}ch + 1.5rem)`;
	const textLayerStyle = multiline ? { paddingLeft: `calc(${lineNumberWidth} + 0.625rem)` } : undefined;
	const variableNames = useMemo(() => new Set(variables.map((variable) => variable.name)), [variables]);
	const completion = getCompletionState(value, caretPosition);
	const suggestions = completion ? getSuggestions(variables, completion.query) : [];
	const showSuggestions = isFocused && !!completion && suggestions.length > 0;
	const lineNumbers = Array.from({ length: lineCount }, (_, index) => index + 1);

	const syncCaret = () => {
		const textarea = textareaRef.current;
		if (textarea) {
			setCaretPosition(textarea.selectionStart);
		}
	};

	const syncScroll = () => {
		if (!multiline || !textareaRef.current) {
			return;
		}

		const { scrollTop, scrollLeft } = textareaRef.current;

		if (highlightRef.current) {
			highlightRef.current.scrollTop = scrollTop;
			highlightRef.current.scrollLeft = scrollLeft;
		}

		if (lineNumberRef.current) {
			lineNumberRef.current.scrollTop = scrollTop;
		}
	};

	const applySuggestion = (completionVariable: VariableCompletion) => {
		if (!completion) {
			return;
		}

		const nextValue = `${value.slice(0, completion.start)}${completionVariable.token}${value.slice(completion.end)}`;
		const nextCaret = completion.start + completionVariable.token.length;
		onChange(nextValue);
		requestAnimationFrame(() => {
			const textarea = textareaRef.current;
			if (!textarea) {
				return;
			}

			textarea.focus();
			textarea.setSelectionRange(nextCaret, nextCaret);
			setCaretPosition(nextCaret);
		});
	};

	const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
		if (event.key === "Tab") {
			event.preventDefault();

			if (showSuggestions && suggestions[0]) {
				applySuggestion(suggestions[0]);
				return;
			}

			const textarea = event.currentTarget;
			const start = textarea.selectionStart;
			const end = textarea.selectionEnd;
			const nextValue = `${value.slice(0, start)}\t${value.slice(end)}`;
			onChange(nextValue);
			requestAnimationFrame(() => {
				textarea.setSelectionRange(start + 1, start + 1);
				setCaretPosition(start + 1);
			});
			return;
		}

		if (event.key === "Enter" && !multiline) {
			if (showSuggestions && suggestions[0]) {
				event.preventDefault();
				applySuggestion(suggestions[0]);
				return;
			}

			event.preventDefault();
		}

		if (event.key === "Enter" && showSuggestions && suggestions[0]) {
			event.preventDefault();
			applySuggestion(suggestions[0]);
		}
	};

	return (
		<div className={cn("relative", containerClassName)}>
			<div
				className={cn(
					"relative overflow-hidden rounded-lg border bg-baud-panel/70 transition-[border-color,box-shadow]",
					!multiline && "grid-cols-1",
					hasError
						? "border-baud-danger shadow-[0_0_0_2px_rgb(224_92_92/0.14)]"
						: "border-baud-border focus-within:border-baud-red/75 focus-within:shadow-[0_0_0_2px_rgb(230_45_62/0.14)]",
					className,
				)}
			>
				{multiline && (
					<div
						ref={lineNumberRef}
						className="pointer-events-none absolute inset-y-0 left-0 z-20 select-none overflow-hidden border-r border-baud-border bg-baud-bg/45 px-2 py-2 text-right font-mono text-sm leading-5 text-baud-muted"
						style={{ width: lineNumberWidth }}
					>
						{lineNumbers.map((lineNumber) => (
							<div key={lineNumber}>{lineNumber}</div>
						))}
					</div>
				)}
				<div className="relative min-w-0">
					<pre
						ref={highlightRef}
						aria-hidden="true"
						style={textLayerStyle}
						className={cn(
							"pointer-events-none absolute inset-0 overflow-hidden whitespace-pre-wrap wrap-break-word px-2.5 py-2 font-mono text-sm leading-5",
							multiline ? "min-h-24" : "h-8 whitespace-pre",
						)}
					>
						{value ? (
							renderHighlightedValue(value, variableNames)
						) : (
							<span className="text-baud-muted">{placeholder}</span>
						)}
						{"\n"}
					</pre>
					<textarea
						id={id}
						ref={textareaRef}
						aria-label={ariaLabel}
						value={value}
						rows={multiline ? lineCount : 1}
						spellCheck={false}
						wrap={multiline ? "soft" : "off"}
						placeholder={placeholder}
						style={textLayerStyle}
						onBlur={() => {
							syncCaret();
							setIsFocused(false);
						}}
						onClick={syncCaret}
						onFocus={() => {
							setIsFocused(true);
							syncCaret();
						}}
						onKeyDown={handleKeyDown}
						onKeyUp={syncCaret}
						onSelect={syncCaret}
						onScroll={syncScroll}
						onChange={(event) => {
							const nextValue = multiline ? event.target.value : event.target.value.replace(/\r?\n/g, " ");
							onChange(nextValue);
							setCaretPosition(Math.min(event.target.selectionStart, nextValue.length));
						}}
						className={cn(
							"relative z-10 block w-full border-0 bg-transparent px-2.5 py-2 font-mono text-sm leading-5 text-transparent caret-baud-text outline-none selection:bg-baud-red/30",
							multiline ? "min-h-24 resize-y overflow-auto" : "h-8 resize-none overflow-hidden whitespace-nowrap",
						)}
					/>
				</div>
			</div>
			{showSuggestions && (
				<div className="absolute z-50 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-baud-border bg-baud-panel p-1 shadow-[0_16px_38px_rgba(0,0,0,0.42)]">
					{suggestions.map((suggestion) => (
						<button
							key={suggestion.name}
							type="button"
							className="flex w-full min-w-0 items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left hover:bg-baud-soft"
							onMouseDown={(event) => {
								event.preventDefault();
								applySuggestion(suggestion);
							}}
						>
							<span className="min-w-0">
								<span className="block truncate font-mono text-sm text-baud-text">{suggestion.token}</span>
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

function getCompletionState(value: string, caretPosition: number): CompletionState | null {
	const beforeCaret = value.slice(0, caretPosition);
	const openIndex = beforeCaret.lastIndexOf("{{");
	const closeIndex = beforeCaret.lastIndexOf("}}");

	if (openIndex === -1 || closeIndex > openIndex) {
		return null;
	}

	const query = beforeCaret.slice(openIndex + 2);
	if (query.includes("{") || query.includes("}") || query.includes("\n")) {
		return null;
	}

	const nextCloseIndex = value.indexOf("}}", caretPosition);
	const nextOpenIndex = value.indexOf("{{", openIndex + 2);
	const shouldReplaceClosingBraces =
		nextCloseIndex !== -1 &&
		(nextOpenIndex === -1 || nextCloseIndex < nextOpenIndex) &&
		!value.slice(caretPosition, nextCloseIndex).includes("\n");

	return {
		start: openIndex,
		end: shouldReplaceClosingBraces ? nextCloseIndex + 2 : caretPosition,
		query: query.trim(),
	};
}

function getSuggestions(variables: VariableCompletion[], query: string) {
	const normalizedQuery = query.toLowerCase();
	return variables
		.filter((variable) => !normalizedQuery || variable.name.toLowerCase().includes(normalizedQuery))
		.slice(0, 12);
}

function renderHighlightedValue(value: string, variableNames: ReadonlySet<string>) {
	const elements: ReactNode[] = [];
	const variablePattern = /\{\{[^{}]*\}\}/g;
	let lastIndex = 0;

	for (const match of value.matchAll(variablePattern)) {
		const start = match.index ?? 0;
		const token = match[0] ?? "";

		if (start > lastIndex) {
			elements.push(<span key={`text-${lastIndex}`}>{value.slice(lastIndex, start)}</span>);
		}

		const name = token.slice(2, -2);
		const normalizedName = name.trim();
		const hasSpacing = name !== normalizedName;
		const known = variableNames.has(normalizedName);

		elements.push(
			<span
				key={`variable-${start}`}
				className={cn(
					"rounded px-0.5",
					known && !hasSpacing && "bg-emerald-400/12 text-emerald-300",
					known && hasSpacing && "bg-amber-400/12 text-amber-300",
					!known && "bg-baud-danger/15 text-baud-danger",
				)}
			>
				{token}
			</span>,
		);

		lastIndex = start + token.length;
	}

	if (lastIndex < value.length) {
		elements.push(<span key={`text-${lastIndex}`}>{value.slice(lastIndex)}</span>);
	}

	return elements;
}
