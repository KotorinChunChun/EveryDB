export type LinkedCredentials = Record<string, { id: number; label: string }>;

let linkedCredentials: LinkedCredentials = {};

export function getLinkedCredentials(): LinkedCredentials {
  return linkedCredentials;
}

export function setLinkedCredentials(value: LinkedCredentials): void {
  linkedCredentials = value;
}

export function clearLinkedCredentials(): void {
  linkedCredentials = {};
}
