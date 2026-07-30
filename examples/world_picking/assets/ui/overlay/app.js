// The overlay's only interactive element. Its counter is the control in this
// demo: whatever we change about picking, superui's own clicks must keep working.
(function () {
  var clicks = 0;
  var button = document.getElementById("ping");

  button.addEventListener("click", function () {
    clicks += 1;
    button.textContent = "superui button - clicks: " + clicks;
  });
})();
