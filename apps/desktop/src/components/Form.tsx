import { RadioGroup, Switch } from 'radix-ui';
import { useId, type ReactNode } from 'react';

export interface ToggleProps {
  label: ReactNode;
  hint?: ReactNode;
  checked: boolean;
  onCheckedChange(checked: boolean): void;
  disabled?: boolean;
}

/** Labelled switch (Radix) with an optional hint linked via `aria-describedby`. */
export function Toggle({ label, hint, checked, onCheckedChange, disabled }: ToggleProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  return (
    <div className="toggle">
      <div className="toggle__text">
        <label className="toggle__label" htmlFor={id}>
          {label}
        </label>
        {hint ? (
          <p className="field__hint" id={hintId}>
            {hint}
          </p>
        ) : null}
      </div>
      <Switch.Root
        id={id}
        className="switch"
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        aria-describedby={hint ? hintId : undefined}
      >
        <Switch.Thumb className="switch__thumb" />
      </Switch.Root>
    </div>
  );
}

export interface RadioOption<T extends string> {
  value: T;
  label: ReactNode;
  hint?: ReactNode;
  badge?: ReactNode;
  disabled?: boolean;
}

export interface RadioCardsProps<T extends string> {
  label: ReactNode;
  value: T;
  options: readonly RadioOption<T>[];
  onValueChange(value: T): void;
  disabled?: boolean;
  /** Content shown directly below a specific option (e.g. an explanation). */
  after?: Partial<Record<T, ReactNode>>;
}

/** Radio group rendered as selectable cards (keyboard: arrow keys). */
export function RadioCards<T extends string>({ label, value, options, onValueChange, disabled, after }: RadioCardsProps<T>) {
  const labelId = useId();
  return (
    <div className="radio-cards">
      <p className="field__label" id={labelId}>
        {label}
      </p>
      <RadioGroup.Root
        className="radio-cards__group"
        value={value}
        onValueChange={(next) => onValueChange(next as T)}
        aria-labelledby={labelId}
        disabled={disabled}
      >
        {options.map((option) => (
          <RadioCardOption key={option.value} option={option} after={after?.[option.value]} />
        ))}
      </RadioGroup.Root>
    </div>
  );
}

function RadioCardOption<T extends string>({ option, after }: { option: RadioOption<T>; after?: ReactNode }) {
  const id = useId();
  return (
    <div className="radio-card-wrap">
      <div className={`radio-card ${option.disabled ? 'radio-card--disabled' : ''}`}>
        <RadioGroup.Item
          className="radio-card__control"
          value={option.value}
          id={id}
          disabled={option.disabled}
          aria-describedby={option.hint ? `${id}-hint` : undefined}
        >
          <RadioGroup.Indicator className="radio-card__indicator" />
        </RadioGroup.Item>
        <div className="radio-card__text">
          <label className="radio-card__label" htmlFor={id}>
            {option.label}
            {option.badge ? <span className="radio-card__badge">{option.badge}</span> : null}
          </label>
          {option.hint ? (
            <p className="field__hint" id={`${id}-hint`}>
              {option.hint}
            </p>
          ) : null}
        </div>
      </div>
      {after ? <div className="radio-card__after">{after}</div> : null}
    </div>
  );
}

export interface ChipGroupProps<T extends string> {
  label: string;
  value: T;
  options: readonly { value: T; label: ReactNode }[];
  onValueChange(value: T): void;
}

/** Single-choice filter chips (radio semantics). */
export function ChipGroup<T extends string>({ label, value, options, onValueChange }: ChipGroupProps<T>) {
  return (
    <RadioGroup.Root
      className="chip-group"
      value={value}
      onValueChange={(next) => onValueChange(next as T)}
      aria-label={label}
      orientation="horizontal"
    >
      {options.map((option) => (
        <RadioGroup.Item key={option.value} value={option.value} className="chip">
          {option.label}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}

export interface SelectFieldProps<T extends string | number> {
  label: ReactNode;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange(value: T): void;
  hint?: ReactNode;
  disabled?: boolean;
}

/** Native select (accessible and keyboard friendly by default). */
export function SelectField<T extends string | number>({ label, value, options, onChange, hint, disabled }: SelectFieldProps<T>) {
  const id = useId();
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        className="select"
        value={String(value)}
        disabled={disabled}
        aria-describedby={hint ? `${id}-hint` : undefined}
        onChange={(event) => {
          const selected = options.find((option) => String(option.value) === event.target.value);
          if (selected) onChange(selected.value);
        }}
      >
        {options.map((option) => (
          <option key={String(option.value)} value={String(option.value)}>
            {option.label}
          </option>
        ))}
      </select>
      {hint ? (
        <p className="field__hint" id={`${id}-hint`}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}

/** Native checkbox with label and optional hint. */
export function Checkbox({
  label,
  hint,
  checked,
  onChange,
  disabled,
}: {
  label: ReactNode;
  hint?: ReactNode;
  checked: boolean;
  onChange(checked: boolean): void;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className="checkbox">
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        aria-describedby={hint ? `${id}-hint` : undefined}
      />
      <div className="checkbox__text">
        <label htmlFor={id}>{label}</label>
        {hint ? (
          <p className="field__hint" id={`${id}-hint`}>
            {hint}
          </p>
        ) : null}
      </div>
    </div>
  );
}
