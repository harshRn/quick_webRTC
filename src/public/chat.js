let divVideoChatLobby = document.getElementById("video-chat-lobby");
let divVideoChat = document.getElementById("video-chat-room");
let joinButton = document.getElementById("join");
let userVideo = document.getElementById("user-video");
let peerVideo = document.getElementById("peer-video");
let roomInput = document.getElementById("roomName");
let socket = null;
let sendMessage = null;
const queue = [];
let creator = null;
let cam_opened = null;

// Contains the stun server URL we will be using.
let iceServers = {
  iceServers: [
    { urls: "stun:stun.services.mozilla.com" },
    { urls: "stun:stun.l.google.com:19302" },
  ],
};

function createSocket(roomName) {
  const ws = new WebSocket(`ws://localhost:9000/ws/${roomName}`);

  function send(msg) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(msg);
    } else {
      queue.push(msg);
    }
  }

  ws.onopen = () => {
    console.log("WebSocket connected");
    // Send any queued messages
    while (queue.length) {
      ws.send(queue.shift());
    }
  };

  ws.onmessage = (e) => {
    // Handle your incoming messages here
    handleMessage(e.data, ws);
  };

  ws.onerror = (e) => {
    console.error("WebSocket error:", e);
  };

  ws.onclose = () => {
    console.log("WebSocket closed");
  };

  return { socket: ws, send };
}

function connect() {
  if (roomInput.value === "") {
    alert("Please enter a room name");
    return;
  }

  divVideoChatLobby.style.display = "none";
  const roomName = roomInput.value;

  const socketObj = createSocket(roomName);
  socket = socketObj.socket;
  sendMessage = socketObj.send;

  // Send your first message
  // sendMessage("first message from client side");
}

// Parse incoming WebSocket message
function parseMessage(rawMessage) {
  try {
    const parsed = JSON.parse(rawMessage);

    // Handle plain string messages
    if (typeof parsed === "string") {
      return {
        type: parsed,
        data: null,
      };
    }

    // Handle object messages
    const messageType = Object.keys(parsed)[0];
    const messageData = JSON.parse(parsed[messageType]);

    return {
      type: messageType,
      data: messageData,
    };
  } catch (error) {
    console.error("Failed to parse message:", error);
    return null;
  }
}

// Route message to appropriate handler
function handleMessage(rawMessage, ws) {
  const message = parseMessage(rawMessage);

  if (!message) return;

  const handler = messageHandlers[message.type];
  if (handler) {
    handler(message.data, ws);
  }
}

function open_cam(created, socket) {
  creator = created;
  navigator.mediaDevices
    .getUserMedia({
      audio: false,
      video: { width: 1280, height: 720 },
    })
    .then(function (stream) {
      /* use the stream */
      userStream = stream;
      divVideoChatLobby.style = "display:none";
      userVideo.srcObject = stream;
      userVideo.onloadedmetadata = function (e) {
        userVideo.play();
      };
      if (creator === false) {
        socket.send('"Ready"');
      }
    })
    .catch(function (err) {
      /* handle the error */
      console.log(err);
      alert("Couldn't Access User Media", err);
    });
  cam_opened = true;
}

function init_rtpc_connection() {
  if (creator) {
    rtcPeerConnection = new RTCPeerConnection(iceServers);
    rtcPeerConnection.onicecandidate = OnIceCandidateFunction;
    rtcPeerConnection.ontrack = OnTrackFunction;
    rtcPeerConnection.addTrack(userStream.getTracks()[0], userStream);
    rtcPeerConnection.addTrack(userStream.getTracks()[1], userStream);
    rtcPeerConnection
      .createOffer()
      .then((offer) => {
        rtcPeerConnection.setLocalDescription(offer);
        socket.emit("offer", offer, roomName); // change
      })

      .catch((error) => {
        console.log(error);
      });
  }
}

// Message handlers
const messageHandlers = {
  Alert: (data, socket) => {
    alert(`${data.connection_result}: IP ${data.ip} in room ${data.room}`);
  },
  Text: (data, socket) => {
    console.log(
      `${data.connection_result}: IP ${data.ip} in room ${data.room}`
    );
    if (cam_opened === null) {
      if (data.connection_result === "Created") {
        open_cam(true, socket);
      } else if (data.connection_result === "Joined") {
        open_cam(false, socket);
      }
    }
  },
  Ready: (data) => {
    init_rtpc_connection();
  },
};

joinButton.addEventListener("click", connect);
// sendMessage("first message from client side");
