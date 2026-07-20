import { createSignal, createMemo, For, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
    </li>
  );
}

function Footer(props) {
  return (
    <div id="footer">
      <span id="count">
        {props.remaining + (props.remaining === 1 ? " item left" : " items left")}
      </span>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [draft, setDraft] = createSignal("");

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };
  const toggle = (id) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <ul id="todo-list">
        {<For each={todos()}>
          {(todo) => <TodoItem todo={todo} onToggle={toggle} />}
        </For>}
      </ul>
      <Footer remaining={remaining()} />
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
