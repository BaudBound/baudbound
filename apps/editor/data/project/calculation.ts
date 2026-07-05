export type CalculationResult = { ok: true; value: number } | { message: string; ok: false };
type TokenizeResult = { ok: true; tokens: Token[] } | { message: string; ok: false };
type OperatorTokenValue = "+" | "-" | "*" | "/" | "%" | "^";
type ParenTokenValue = "(" | ")";

type Token =
	| { type: "comma"; value: "," }
	| { type: "identifier"; value: string }
	| { type: "number"; value: number }
	| { type: "operator"; value: OperatorTokenValue }
	| { type: "paren"; value: ParenTokenValue };

export function validateCalculationExpression(expression: string) {
	const trimmed = expression.trim();

	if (!trimmed) {
		return "Expression is required.";
	}

	if (containsTemplateReference(trimmed)) {
		return "";
	}

	const result = evaluateCalculationExpression(trimmed);
	return result.ok ? "" : result.message;
}

export function evaluateCalculationExpression(expression: string): CalculationResult {
	const tokens = tokenizeExpression(expression);
	if (!tokens.ok) {
		return tokens;
	}

	const parser = new CalculationParser(tokens.tokens);
	const result = parser.parseExpression();
	if (!result.ok) {
		return result;
	}

	if (!parser.isComplete()) {
		return { ok: false, message: "Expression contains trailing tokens." };
	}

	return Number.isFinite(result.value) ? result : { ok: false, message: "Expression result must be finite." };
}

function tokenizeExpression(expression: string): TokenizeResult {
	const tokens: Token[] = [];
	let index = 0;

	while (index < expression.length) {
		const char = expression[index];
		if (!char) {
			break;
		}

		if (/\s/.test(char)) {
			index += 1;
			continue;
		}

		if (char === "(" || char === ")") {
			tokens.push({ type: "paren", value: char });
			index += 1;
			continue;
		}

		if (char === ",") {
			tokens.push({ type: "comma", value: char });
			index += 1;
			continue;
		}

		if (isOperator(char)) {
			tokens.push({ type: "operator", value: char });
			index += 1;
			continue;
		}

		const numberMatch = expression.slice(index).match(/^(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/i);
		if (numberMatch?.[0]) {
			const value = Number(numberMatch[0]);
			if (!Number.isFinite(value)) {
				return { ok: false, message: `Invalid number "${numberMatch[0]}".` };
			}

			tokens.push({ type: "number", value });
			index += numberMatch[0].length;
			continue;
		}

		const identifierMatch = expression.slice(index).match(/^[a-z_][a-z0-9_]*/i);
		if (identifierMatch?.[0]) {
			tokens.push({ type: "identifier", value: identifierMatch[0].toLowerCase() });
			index += identifierMatch[0].length;
			continue;
		}

		return { ok: false, message: `Unexpected token "${char}".` };
	}

	return tokens.length > 0 ? { ok: true, tokens } : { ok: false, message: "Expression is required." };
}

class CalculationParser {
	private index = 0;

	constructor(private readonly tokens: Token[]) {}

	isComplete() {
		return this.index >= this.tokens.length;
	}

	parseExpression(): CalculationResult {
		let left = this.parseTerm();
		if (!left.ok) {
			return left;
		}

		while (this.matchOperator("+") || this.matchOperator("-")) {
			const operator = this.previous()?.value;
			const right = this.parseTerm();
			if (!right.ok) {
				return right;
			}

			left = {
				ok: true,
				value: operator === "+" ? left.value + right.value : left.value - right.value,
			};
		}

		return left;
	}

	private parseTerm(): CalculationResult {
		let left = this.parseUnary();
		if (!left.ok) {
			return left;
		}

		while (this.matchOperator("*") || this.matchOperator("/") || this.matchOperator("%")) {
			const operator = this.previous()?.value;
			const right = this.parseUnary();
			if (!right.ok) {
				return right;
			}

			if ((operator === "/" || operator === "%") && right.value === 0) {
				return { ok: false, message: "Division by zero is not allowed." };
			}

			left = {
				ok: true,
				value:
					operator === "*"
						? left.value * right.value
						: operator === "/"
							? left.value / right.value
							: left.value % right.value,
			};
		}

		return left;
	}

	private parseUnary(): CalculationResult {
		if (this.matchOperator("-")) {
			const value = this.parseUnary();
			return value.ok ? { ok: true, value: -value.value } : value;
		}

		if (this.matchOperator("+")) {
			return this.parseUnary();
		}

		return this.parsePower();
	}

	private parsePower(): CalculationResult {
		const left = this.parsePrimary();
		if (!left.ok) {
			return left;
		}

		if (!this.matchOperator("^")) {
			return left;
		}

		const right = this.parseUnary();
		if (!right.ok) {
			return right;
		}

		const value = left.value ** right.value;
		return Number.isFinite(value) ? { ok: true, value } : { ok: false, message: "Exponent result must be finite." };
	}

	private parsePrimary(): CalculationResult {
		const token = this.advance();
		if (!token) {
			return { ok: false, message: "Expression ended unexpectedly." };
		}

		if (token.type === "number") {
			return { ok: true, value: token.value };
		}

		if (token.type === "paren" && token.value === "(") {
			const expression = this.parseExpression();
			if (!expression.ok) {
				return expression;
			}

			if (!this.matchParen(")")) {
				return { ok: false, message: "Missing closing parenthesis." };
			}

			return expression;
		}

		if (token.type === "identifier") {
			return this.parseFunctionCall(token.value);
		}

		return { ok: false, message: `Unexpected ${token.type}.` };
	}

	private parseFunctionCall(name: string): CalculationResult {
		if (!this.matchParen("(")) {
			return { ok: false, message: `Function "${name}" must be called with parentheses.` };
		}

		const args: number[] = [];
		if (!this.matchParen(")")) {
			do {
				const arg = this.parseExpression();
				if (!arg.ok) {
					return arg;
				}
				args.push(arg.value);
			} while (this.matchComma());

			if (!this.matchParen(")")) {
				return { ok: false, message: `Function "${name}" is missing a closing parenthesis.` };
			}
		}

		return evaluateFunction(name, args);
	}

	private matchOperator(value: OperatorTokenValue) {
		const token = this.peek();
		if (token?.type !== "operator" || token.value !== value) {
			return false;
		}

		this.index += 1;
		return true;
	}

	private matchParen(value: ParenTokenValue) {
		const token = this.peek();
		if (token?.type !== "paren" || token.value !== value) {
			return false;
		}

		this.index += 1;
		return true;
	}

	private matchComma() {
		const token = this.peek();
		if (token?.type !== "comma") {
			return false;
		}

		this.index += 1;
		return true;
	}

	private advance() {
		const token = this.peek();
		if (token) {
			this.index += 1;
		}

		return token;
	}

	private peek() {
		return this.tokens[this.index];
	}

	private previous() {
		return this.tokens[this.index - 1];
	}
}

function evaluateFunction(name: string, args: number[]): CalculationResult {
	switch (name) {
		case "round":
		case "floor":
		case "ceil":
			if (args.length !== 1) {
				return { ok: false, message: `${name}() expects exactly one argument.` };
			}
			return {
				ok: true,
				value:
					name === "round"
						? Math.round(args[0] ?? 0)
						: name === "floor"
							? Math.floor(args[0] ?? 0)
							: Math.ceil(args[0] ?? 0),
			};
		case "min":
		case "max":
			if (args.length === 0) {
				return { ok: false, message: `${name}() expects at least one argument.` };
			}
			return { ok: true, value: name === "min" ? Math.min(...args) : Math.max(...args) };
		case "random":
			if (args.length > 2) {
				return { ok: false, message: "random() expects zero, one, or two arguments." };
			}
			if (args.length === 0) {
				return { ok: true, value: Math.random() };
			}
			if (args.length === 1) {
				return { ok: true, value: Math.random() * (args[0] ?? 0) };
			}
			return { ok: true, value: (args[0] ?? 0) + Math.random() * ((args[1] ?? 0) - (args[0] ?? 0)) };
		default:
			return { ok: false, message: `Unknown function "${name}".` };
	}
}

function containsTemplateReference(value: string) {
	return /\{\{\s*[^{}]+\s*\}\}/.test(value);
}

function isOperator(value: string): value is OperatorTokenValue {
	return value === "+" || value === "-" || value === "*" || value === "/" || value === "%" || value === "^";
}
