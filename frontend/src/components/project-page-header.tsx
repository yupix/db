export function ProjectPageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: string;
  actions?: React.ReactNode;
}) {
  return (
    <div className="sticky top-0 z-10 bg-background/80 backdrop-blur border-b">
      <div className="px-6 h-16 flex items-center justify-between gap-4">
        <div className="min-w-0">
          <h1 className="text-lg font-bold truncate">{title}</h1>
          {description && (
            <p className="text-xs text-muted-foreground mt-0.5 truncate">{description}</p>
          )}
        </div>
        {actions && <div className="flex gap-2 shrink-0">{actions}</div>}
      </div>
    </div>
  );
}
