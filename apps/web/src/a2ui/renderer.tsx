import { useEffect, useMemo, useState, type FormEvent } from "react";

import { SanitizedMarkdown } from "./markdown";
import type {
  A2uiChartComponent,
  A2uiComponent,
  A2uiDocument,
  A2uiFormComponent,
  A2uiFormField,
  A2uiFormSubmitEvent,
  A2uiFormValue
} from "./types";

interface A2uiRendererProps {
  readonly document: A2uiDocument;
  readonly onFormSubmit?: (event: A2uiFormSubmitEvent) => void;
}

export function A2uiRenderer({ document, onFormSubmit }: A2uiRendererProps) {
  return (
    <section className="a2ui-renderer" data-surface={document.surface} aria-label={document.surface}>
      {document.components.map((component, index) => (
        <article
          key={component.id}
          className="a2ui-component"
          data-component-id={component.id}
          data-component-type={component.type}
          style={{ animationDelay: `${index * 32}ms` }}
        >
          <ComponentBody component={component} onFormSubmit={onFormSubmit} />
        </article>
      ))}
      {document.components.length === 0 ? (
        <article className="a2ui-component a2ui-component-empty">No renderable components.</article>
      ) : null}
    </section>
  );
}

interface ComponentBodyProps {
  readonly component: A2uiComponent;
  readonly onFormSubmit?: (event: A2uiFormSubmitEvent) => void;
}

function ComponentBody({ component, onFormSubmit }: ComponentBodyProps) {
  switch (component.type) {
    case "text":
      return <p className={`a2ui-text a2ui-text-${component.props.tone}`}>{component.props.value}</p>;
    case "markdown":
      return <SanitizedMarkdown value={component.props.value} />;
    case "list":
      return component.props.ordered ? (
        <ol className="a2ui-list">
          {component.props.items.map((item, index) => (
            <li key={`${component.id}-${index}`}>{item}</li>
          ))}
        </ol>
      ) : (
        <ul className="a2ui-list">
          {component.props.items.map((item, index) => (
            <li key={`${component.id}-${index}`}>{item}</li>
          ))}
        </ul>
      );
    case "table":
      return (
        <div className="a2ui-table-wrap">
          <table className="a2ui-table">
            <thead>
              <tr>
                {component.props.columns.map((column) => (
                  <th key={`${component.id}-${column}`}>{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {component.props.rows.map((row, rowIndex) => (
                <tr key={`${component.id}-row-${rowIndex}`}>
                  {row.map((cell, cellIndex) => (
                    <td key={`${component.id}-cell-${rowIndex}-${cellIndex}`}>{cell}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    case "form":
      return <A2uiForm component={component} onSubmit={onFormSubmit} />;
    case "chart":
      return <A2uiBarChart component={component} />;
    default:
      return null;
  }
}

interface A2uiFormProps {
  readonly component: A2uiFormComponent;
  readonly onSubmit?: (event: A2uiFormSubmitEvent) => void;
}

function A2uiForm({ component, onSubmit }: A2uiFormProps) {
  const initialValues = useMemo(
    () => buildInitialFormValues(component.props.fields),
    [component.props.fields]
  );
  const [values, setValues] = useState<Record<string, A2uiFormValue>>(initialValues);

  useEffect(() => {
    setValues(initialValues);
  }, [initialValues]);

  function updateFieldValue(fieldId: string, value: A2uiFormValue): void {
    setValues((current) => ({
      ...current,
      [fieldId]: value
    }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    onSubmit?.({
      componentId: component.id,
      values
    });
  }

  return (
    <form className="a2ui-form" onSubmit={handleSubmit}>
      <header className="a2ui-form-header">
        <h3>{component.props.title}</h3>
      </header>
      <div className="a2ui-form-fields">
        {component.props.fields.map((field) => (
          <FormFieldRow
            key={`${component.id}-${field.id}`}
            componentId={component.id}
            field={field}
            value={values[field.id]}
            onChange={updateFieldValue}
          />
        ))}
      </div>
      <footer className="a2ui-form-footer">
        <button type="submit">{component.props.submitLabel}</button>
      </footer>
    </form>
  );
}

interface FormFieldRowProps {
  readonly componentId: string;
  readonly field: A2uiFormField;
  readonly value: A2uiFormValue | undefined;
  readonly onChange: (fieldId: string, value: A2uiFormValue) => void;
}

function FormFieldRow({ componentId, field, value, onChange }: FormFieldRowProps) {
  const inputId = `${componentId}-${field.id}`;

  if (field.type === "checkbox") {
    const checked = typeof value === "boolean" ? value : field.defaultValue;
    return (
      <label htmlFor={inputId} className="a2ui-form-field a2ui-form-field-checkbox">
        <input
          id={inputId}
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(field.id, event.currentTarget.checked)}
        />
        <span>{field.label}</span>
      </label>
    );
  }

  if (field.type === "select") {
    const selectedValue = typeof value === "string" ? value : field.defaultValue;
    return (
      <label htmlFor={inputId} className="a2ui-form-field">
        <span>{field.label}</span>
        <select
          id={inputId}
          value={selectedValue}
          required={field.required}
          onChange={(event) => onChange(field.id, event.currentTarget.value)}
        >
          {field.options.map((option) => (
            <option key={`${field.id}-${option.value}`} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {field.hint.length > 0 ? <small>{field.hint}</small> : null}
      </label>
    );
  }

  if (field.type === "number") {
    const numberValue = typeof value === "number" ? value : field.defaultValue;
    return (
      <label htmlFor={inputId} className="a2ui-form-field">
        <span>{field.label}</span>
        <input
          id={inputId}
          type="number"
          min={field.min}
          max={field.max}
          step={field.step}
          required={field.required}
          value={numberValue}
          onChange={(event) => {
            const parsed = Number.parseFloat(event.currentTarget.value);
            onChange(field.id, Number.isFinite(parsed) ? parsed : field.defaultValue);
          }}
        />
        {field.hint.length > 0 ? <small>{field.hint}</small> : null}
      </label>
    );
  }

  const textValue = typeof value === "string" ? value : field.defaultValue;
  return (
    <label htmlFor={inputId} className="a2ui-form-field">
      <span>{field.label}</span>
      <input
        id={inputId}
        type={field.type}
        value={textValue}
        placeholder={field.placeholder}
        required={field.required}
        onChange={(event) => onChange(field.id, event.currentTarget.value)}
      />
      {field.hint.length > 0 ? <small>{field.hint}</small> : null}
    </label>
  );
}

function buildInitialFormValues(fields: readonly A2uiFormField[]): Record<string, A2uiFormValue> {
  const values: Record<string, A2uiFormValue> = {};
  for (const field of fields) {
    if (field.type === "checkbox") {
      values[field.id] = field.defaultValue;
      continue;
    }
    if (field.type === "number") {
      values[field.id] = field.defaultValue;
      continue;
    }
    if (field.type === "select") {
      values[field.id] = field.defaultValue;
      continue;
    }
    values[field.id] = field.defaultValue;
  }
  return values;
}

interface A2uiBarChartProps {
  readonly component: A2uiChartComponent;
}

function A2uiBarChart({ component }: A2uiBarChartProps) {
  const maxValue = component.props.series.reduce((maximum, entry) => Math.max(maximum, entry.value), 1);

  return (
    <figure className="a2ui-chart">
      <figcaption>{component.props.title}</figcaption>
      <div className="a2ui-chart-bars" role="img" aria-label={component.props.title}>
        {component.props.series.map((entry) => {
          const width = `${Math.min(100, (entry.value / maxValue) * 100)}%`;
          return (
            <div key={`${component.id}-${entry.label}`} className="a2ui-chart-row">
              <span className="a2ui-chart-label">{entry.label}</span>
              <span className="a2ui-chart-track">
                <span className="a2ui-chart-bar" style={{ width }} />
              </span>
              <span className="a2ui-chart-value">{entry.value}</span>
            </div>
          );
        })}
      </div>
    </figure>
  );
}
