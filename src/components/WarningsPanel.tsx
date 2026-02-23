export function WarningsPanel({ warnings }: { warnings: Array<{ code: string; message: string }> }) {
  return (
    <div>
      <h3>Warnings</h3>
      {warnings.length === 0 ? <p>No warnings.</p> : (
        <ul>
          {warnings.map((w, i) => <li key={`${w.code}-${i}`}>{w.code}: {w.message}</li>)}
        </ul>
      )}
    </div>
  );
}
