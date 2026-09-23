(function () {
  var subscribe = document.getElementById("subscribe");
  var subscribeReadout = document.getElementById("subscribe-readout");
  subscribe.addEventListener("change", function (e) {
    subscribeReadout.textContent = e.target.checked ? "on" : "off";
  });

  var volume = document.getElementById("volume");
  var volumeReadout = document.getElementById("volume-readout");
  volume.addEventListener("input", function (e) {
    volumeReadout.textContent = e.target.value;
  });
})();
