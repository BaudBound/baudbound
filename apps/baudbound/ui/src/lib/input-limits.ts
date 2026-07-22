export const SEARCH_INPUT_MAX_LENGTH = 256;
export const SECRET_INPUT_MAX_LENGTH = 1_048_576;
export const RUNNER_CONFIG_MAX_BYTES = 1_048_576;
export const RUNNER_TEXT_INPUT_MAX_LENGTH = 1_024;
export const SERIAL_DEVICE_ID_MAX_LENGTH = 64;
export const SERIAL_METADATA_MAX_LENGTH = 512;
export const BROWSER_ORIGIN_MAX_LENGTH = 2_048;
export const BROWSER_ORIGIN_MAX_COUNT = 128;
export const BIND_ADDRESS_MAX_LENGTH = 255;

export function utf8Length(value: string) {
  return new TextEncoder().encode(value).length;
}
