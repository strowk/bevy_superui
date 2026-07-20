import { render } from "supersolid";

function App() {
  return (
    <div id="app">
      <h1>todos</h1>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
