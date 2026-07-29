import { createSignal, createMemo, For, Show, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }
type Filter = "all" | "active" | "completed";

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" class="w-[220px]" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
      <button class="destroy" onClick={() => props.onRemove(props.todo.id)}>x</button>
    </li>
  );
}

function Footer(props) {
  return (
    <div id="footer">
      <span id="count">
        {props.remaining + (props.remaining === 1 ? " item left" : " items left")}
      </span>
      <div class="filters">
        <button id="filter-all" class={props.filter === "all" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("all")}>All</button>
        <button id="filter-active" class={props.filter === "active" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("active")}>Active</button>
        <button id="filter-completed"
                class={props.filter === "completed" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("completed")}>Completed</button>
      </div>
      <button id="clear-completed" class="clear-completed"
              onClick={() => props.onClearCompleted()}>Clear completed</button>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [draft, setDraft] = createSignal("");

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);
  const filtered = createMemo(() => {
    const f = filter();
    return todos().filter((t) => (f === "all" ? true : f === "active" ? !t.done : t.done));
  });

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };
  const toggle = (id) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  const remove = (id) => setTodos(todos().filter((t) => t.id !== id));
  const clearCompleted = () => setTodos(todos().filter((t) => !t.done));
  const toggleAll = () => {
    const allDone = todos().length > 0 && todos().every((t) => t.done);
    setTodos(todos().map((t) => ({ ...t, done: !allDone })));
  };

  return (
    <div id="app">
      <h1 class="pt-4 text-center italic bg-slate-200">todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <div id="main">
        <input id="toggle-all" type="checkbox"
               checked={todos().length > 0 && todos().every((t) => t.done)}
               onChange={() => toggleAll()} />
        <ul id="todo-list">
          <For each={filtered()}>
            {(todo) => <TodoItem todo={todo} onToggle={toggle} onRemove={remove} />}
          </For>
        </ul>
      </div>
      <Show when={todos().length > 0}>
        <Footer remaining={remaining()} filter={filter()}
                onFilter={setFilter} onClearCompleted={clearCompleted} />
      </Show>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
