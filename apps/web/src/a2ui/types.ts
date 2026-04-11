export type JsonPrimitive = string | number | boolean | null;

export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];

export interface JsonObject {
  [key: string]: JsonValue;
}

export type A2uiComponentType = "text" | "markdown" | "list" | "table" | "form" | "chart";

export type A2uiExperimentRolloutStage =
  | "disabled"
  | "dark_launch"
  | "operator_preview"
  | "limited_preview";

export type A2uiExperimentAmbientMode = "disabled" | "push_to_talk";

export interface A2uiExperimentGovernance {
  readonly trackId: string;
  readonly featureFlag: string;
  readonly rolloutStage: A2uiExperimentRolloutStage;
  readonly ambientMode: A2uiExperimentAmbientMode;
  readonly consentRequired: boolean;
  readonly supportSummary: string;
  readonly securityReview: readonly string[];
  readonly exitCriteria: readonly string[];
}

export interface A2uiDocument {
  readonly v: 1;
  readonly surface: string;
  readonly components: readonly A2uiComponent[];
  readonly experimental?: A2uiExperimentGovernance;
}

interface A2uiBaseComponent<TType extends A2uiComponentType, TProps> {
  readonly id: string;
  readonly type: TType;
  readonly props: TProps;
}

export interface A2uiTextProps {
  readonly value: string;
  readonly tone: "normal" | "muted" | "success" | "critical";
}

export type A2uiTextComponent = A2uiBaseComponent<"text", A2uiTextProps>;

export interface A2uiMarkdownProps {
  readonly value: string;
}

export type A2uiMarkdownComponent = A2uiBaseComponent<"markdown", A2uiMarkdownProps>;

export interface A2uiListProps {
  readonly ordered: boolean;
  readonly items: readonly string[];
}

export type A2uiListComponent = A2uiBaseComponent<"list", A2uiListProps>;

export interface A2uiTableProps {
  readonly columns: readonly string[];
  readonly rows: readonly (readonly string[])[];
}

export type A2uiTableComponent = A2uiBaseComponent<"table", A2uiTableProps>;

export type A2uiFormFieldType = "text" | "email" | "number" | "select" | "checkbox";

interface A2uiBaseFormField<TType extends A2uiFormFieldType> {
  readonly id: string;
  readonly label: string;
  readonly type: TType;
  readonly hint: string;
  readonly required: boolean;
}

export interface A2uiTextFormField extends A2uiBaseFormField<"text" | "email"> {
  readonly placeholder: string;
  readonly defaultValue: string;
}

export interface A2uiNumberFormField extends A2uiBaseFormField<"number"> {
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly defaultValue: number;
}

export interface A2uiSelectOption {
  readonly label: string;
  readonly value: string;
}

export interface A2uiSelectFormField extends A2uiBaseFormField<"select"> {
  readonly options: readonly A2uiSelectOption[];
  readonly defaultValue: string;
}

export interface A2uiCheckboxFormField extends A2uiBaseFormField<"checkbox"> {
  readonly defaultValue: boolean;
}

export type A2uiFormField =
  | A2uiTextFormField
  | A2uiNumberFormField
  | A2uiSelectFormField
  | A2uiCheckboxFormField;

export type A2uiFormValue = string | number | boolean;

export interface A2uiFormProps {
  readonly title: string;
  readonly submitLabel: string;
  readonly fields: readonly A2uiFormField[];
}

export type A2uiFormComponent = A2uiBaseComponent<"form", A2uiFormProps>;

export interface A2uiChartSeriesPoint {
  readonly label: string;
  readonly value: number;
}

export interface A2uiChartProps {
  readonly title: string;
  readonly series: readonly A2uiChartSeriesPoint[];
}

export type A2uiChartComponent = A2uiBaseComponent<"chart", A2uiChartProps>;

export type A2uiComponent =
  | A2uiTextComponent
  | A2uiMarkdownComponent
  | A2uiListComponent
  | A2uiTableComponent
  | A2uiFormComponent
  | A2uiChartComponent;

export type PatchOperationKind = "add" | "replace" | "remove";

interface PatchOperationBase<TType extends PatchOperationKind> {
  readonly op: TType;
  readonly path: string;
}

export type AddPatchOperation = PatchOperationBase<"add"> & { readonly value: JsonValue };

export type ReplacePatchOperation = PatchOperationBase<"replace"> & { readonly value: JsonValue };

export type RemovePatchOperation = PatchOperationBase<"remove">;

export type PatchOperation = AddPatchOperation | ReplacePatchOperation | RemovePatchOperation;

export interface PatchDocument {
  readonly v: 1;
  readonly ops: readonly PatchOperation[];
}

export interface RenderInputLimits {
  readonly maxSurfaceLength: number;
  readonly maxComponents: number;
  readonly maxComponentIdLength: number;
  readonly maxStringLength: number;
  readonly maxMarkdownLength: number;
  readonly maxListItems: number;
  readonly maxTableColumns: number;
  readonly maxTableRows: number;
  readonly maxFormFields: number;
  readonly maxSelectOptions: number;
  readonly maxChartPoints: number;
}

export interface PatchProcessingBudget {
  readonly maxOpsPerPatch: number;
  readonly maxOpsPerTick: number;
  readonly maxQueueDepth: number;
  readonly maxPathLength: number;
  readonly maxApplyMsPerTick: number;
}

export interface PatchProcessingResult {
  readonly nextDocument: A2uiDocument;
  readonly appliedPatches: number;
  readonly remainingPatches: readonly PatchDocument[];
  readonly exhaustedBudget: boolean;
  readonly elapsedMs: number;
}

export interface A2uiFormSubmitEvent {
  readonly componentId: string;
  readonly values: Readonly<Record<string, A2uiFormValue>>;
}

export function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isJsonValue(value: unknown): value is JsonValue {
  return isJsonValueInternal(value, new Set<unknown>());
}

function isJsonValueInternal(value: unknown, visited: Set<unknown>): value is JsonValue {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return true;
  }
  if (Array.isArray(value)) {
    if (visited.has(value)) {
      return false;
    }
    visited.add(value);
    return value.every((entry) => isJsonValueInternal(entry, visited));
  }
  if (isJsonObject(value)) {
    if (visited.has(value)) {
      return false;
    }
    visited.add(value);
    return Object.values(value).every((entry) => isJsonValueInternal(entry, visited));
  }
  return false;
}
