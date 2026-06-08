# Skill: React Components

**Trigger:** react, component, JSX, useState, useEffect, hook

**Description:** Développement React : composants, hooks, state management, performance, patterns.

## Body

### Composant standard (TypeScript)
```tsx
interface Props {
  title: string;
  onAction: (id: string) => void;
}

export function TaskCard({ title, onAction }: Props) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="card" onClick={() => setExpanded(!expanded)}>
      <h3>{title}</h3>
      {expanded && <TaskDetails />}
    </div>
  );
}
```

### Hooks essentiels
```tsx
// useState — état local
const [count, setCount] = useState(0);

// useEffect — side effects
useEffect(() => {
  fetchData().then(setData);
  return () => { /* cleanup */ };
}, [dependency]);

// useMemo — éviter les recalculs
const sorted = useMemo(() => items.sort(), [items]);

// useCallback — stabiliser les références
const handleClick = useCallback((id) => {
  doSomething(id);
}, []);
```

### Patterns
```tsx
// Custom hook
function useFetch<T>(url: string) {
  const [data, setData] = useState<T | null>(null);
  useEffect(() => { fetch(url).then(r => r.json()).then(setData); }, [url]);
  return data;
}

// Error boundary
class ErrorBoundary extends React.Component {
  state = { error: null };
  static getDerivedStateFromError(e: Error) { return { error: e }; }
  render() { return this.state.error ? <ErrorView /> : this.props.children; }
}
```

### Pièges
- `useEffect` sans tableau de dépendances → boucle infinie
- État muté directement : `arr.push(x)` → `setArr([...arr, x])`
- Closure stale dans `useEffect` → utiliser la forme fonctionnelle
