(function(){
  const wsProtocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(`${wsProtocol}//${location.hostname}:3000/ws`);
  const mien = document.getElementById('mien');
  const words = document.getElementById('words');
  const thought = document.getElementById('thought');
  function playAudio(b64){
    const audio = new Audio(`data:audio/wav;base64,${b64}`);
    audio.play();
  }
  ws.onmessage = (ev)=>{
    try{
      const m = JSON.parse(ev.data);
      switch(m.type){
        case 'Emote':
        case 'emote':
          mien.textContent = m.data;
          break;
        case 'Say':
        case 'say':
          words.textContent += '\n' + m.data.words;
          if(m.data.audio){ playAudio(m.data.audio); }
          break;
        case 'Think':
        case 'think':
          thought.textContent = m.data;
          break;
        case 'Heard':
        case 'heard':
          // ignore for now
          break;
      }
    }catch(e){console.error(e);}
  };

  document.getElementById('text-form').addEventListener('submit', (e)=>{
    e.preventDefault();
    const input=document.getElementById('text-input');
    const text=input.value.trim();
    if(text){
      ws.send(JSON.stringify({type:'Text', data:text}));
      input.value='';
    }
  });

  if(navigator.geolocation){
    navigator.geolocation.watchPosition(pos=>{
      ws.send(JSON.stringify({type:'Geolocate', data:{longitude:pos.coords.longitude, latitude:pos.coords.latitude}}));
    });
  }

  async function setupWebcam(){
    try{
      const video=document.getElementById('webcam');
      const stream=await navigator.mediaDevices.getUserMedia({video:true});
      video.srcObject=stream;
      const canvas=document.createElement('canvas');
      setInterval(()=>{
        if(video.videoWidth===0) return;
        canvas.width=video.videoWidth; canvas.height=video.videoHeight;
        canvas.getContext('2d').drawImage(video,0,0);
        const data=canvas.toDataURL('image/jpeg');
        ws.send(JSON.stringify({type:'See', data:data}));
      },1000);
    }catch(e){console.error('webcam',e);}
  }
  if(navigator.mediaDevices && navigator.mediaDevices.getUserMedia){
    setupWebcam();
  }

  async function setupAudio(){
    try{
      const stream=await navigator.mediaDevices.getUserMedia({audio:true});
      const rec=new MediaRecorder(stream);
      rec.ondataavailable=e=>{
        if(e.data.size>0){
          const reader=new FileReader();
          reader.onloadend=()=>{
            const base64=reader.result.split(',')[1];
            ws.send(JSON.stringify({type:'Hear', data:{base64:base64, mime:e.data.type}}));
          };
          reader.readAsDataURL(e.data);
        }
      };
      rec.start(1000);
    }catch(e){console.error('audio',e);}
  }
  if(navigator.mediaDevices && navigator.mediaDevices.getUserMedia){
    setupAudio();
  }
})();
