// TodoMVC on bevy_superui. Authored in plain DOM/JS against the implemented
// subset (no `.key` on key events, so we add via the Add button, not Enter).

(function () {
  var todos = []; // { id, label, done }
  var nextId = 1;
  var filter = "all"; // all | active | completed

  var input = document.getElementById("new-todo");
  var list = document.getElementById("todo-list");
  var count = document.getElementById("count");

  function visible(t) {
    if (filter === "active") return !t.done;
    if (filter === "completed") return t.done;
    return true;
  }

  function render() {
    // Rebuild the list from state (simple + correct for Phase 1).
    while (list.firstChild) list.removeChild(list.firstChild);

    for (var i = 0; i < todos.length; i++) {
      var t = todos[i];
      if (!visible(t)) continue;

      var li = document.createElement("li");
      li.className = t.done ? "todo completed" : "todo";
      li.setAttribute("data-id", String(t.id));

      var toggle = document.createElement("input");
      toggle.setAttribute("type", "checkbox");
      toggle.className = "toggle";
      if (t.done) toggle.checked = true;
      toggle.addEventListener("change", makeToggleHandler(t.id));

      var label = document.createElement("span");
      label.className = "label";
      label.textContent = t.label;

      var destroy = document.createElement("button");
      destroy.className = "destroy";
      destroy.textContent = "x";
      destroy.addEventListener("click", makeDestroyHandler(t.id));

      li.appendChild(toggle);
      li.appendChild(label);
      li.appendChild(destroy);
      list.appendChild(li);
    }

    var left = todos.filter(function (t) { return !t.done; }).length;
    count.textContent = left + (left === 1 ? " item left" : " items left");
  }

  function makeToggleHandler(id) {
    return function () {
      for (var i = 0; i < todos.length; i++) {
        if (todos[i].id === id) { todos[i].done = !todos[i].done; break; }
      }
      render();
    };
  }
  function makeDestroyHandler(id) {
    return function () {
      todos = todos.filter(function (t) { return t.id !== id; });
      render();
    };
  }

  function addTodo() {
    var label = (input.value || "").trim();
    if (!label) return;
    todos.push({ id: nextId++, label: label, done: false });
    input.value = "";
    bevy.send("TodoAdded", { label: label }); // demo the ECS seam (design §9)
    render();
  }

  document.getElementById("add").addEventListener("click", addTodo);

  // Filters.
  function setFilter(name) {
    filter = name;
    var buttons = document.querySelectorAll(".filter");
    for (var i = 0; i < buttons.length; i++) buttons[i].classList.remove("selected");
    document.getElementById("filter-" + name).classList.add("selected");
    render();
  }
  document.getElementById("filter-all").addEventListener("click", function () { setFilter("all"); });
  document.getElementById("filter-active").addEventListener("click", function () { setFilter("active"); });
  document.getElementById("filter-completed").addEventListener("click", function () { setFilter("completed"); });

  render();
})();
