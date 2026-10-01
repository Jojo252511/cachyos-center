/** A message with plural forms (German and English only need `one`/`other`). */
export interface PluralMessage {
  one: string;
  other: string;
}

export type Message = string | PluralMessage;

export type Language = 'de' | 'en';
