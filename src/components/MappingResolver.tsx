export function MappingResolver({ unresolved }: { unresolved: Array<{ preregVar: string; candidates: Array<{ key: string; score: number }> }> }) {
  return (
    <div>
      <h3>Resolve Variable Mappings</h3>
      {unresolved.length === 0 && <p>No unresolved mappings.</p>}
      {unresolved.map((u) => (
        <div key={u.preregVar}>
          <strong>{u.preregVar}</strong>
          <ul>
            {u.candidates.map((c) => (
              <li key={c.key}>{c.key} ({c.score.toFixed(2)})</li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}
