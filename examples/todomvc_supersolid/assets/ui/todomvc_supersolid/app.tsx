import { createSignal, For, render } from "supersolid";

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

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [draft, setDraft] = createSignal("");

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <ul id="todo-list">
        {<For each={todos()}>
          {(todo) => (
            <li class="todo" data-id={todo.id}>
              <span class="label">{todo.title}</span>
            </li>
          )}
        </For>}
      </ul>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
