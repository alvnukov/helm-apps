export interface ValidationIssue {
  code: string;
  message: string;
  path: string;
  line: number;
}

export interface ValidationOptions {
  allowNativeListsInBuiltInListFields?: boolean;
}
