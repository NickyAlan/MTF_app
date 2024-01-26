const { tempdir } = window.__TAURI__.os;
const { invoke } = window.__TAURI__.tauri;
const { open } = window.__TAURI__.dialog;
const { convertFileSrc } = window.__TAURI__.tauri;

// home.html
let openFile;
let inputName;
let inputRoom;
let processBtn;
let filePathsImage = []; // temporary store file paths

function openFilefn() {
    return new Promise((resolve, reject) => {
        open({
            multiple: true,
            title: "Open DICOM file",
            filters: [{
                name: 'DICOM',
                extensions: ["*"]
            }]
        }).then((filePaths) => {
            if (filePaths) {
                resolve(filePaths);
            } else {
                reject("No file selected");
            }
        }).catch(reject);
    });
}

async function readFile() {
    const filePaths = await openFilefn();
    if (filePaths) {
        filePathsImage = filePaths;
    } 
};

async function processing() {
    const tempDir = await tempdir();
    let filePaths = filePathsImage;
    if (filePaths.length > 0) {
        let userName = inputName.value;
        let room = inputRoom.value;
        let savePath = `${tempDir}MTFhomedetails.txt`;
        let content = `${userName}\n${room}`;
        for (let path of filePaths) {
            content += `\n${path}`
        };
        await invoke("write_file", {content: content, savePath: savePath});
        // home -> index
        await invoke("home2processing");
        filePathsImage = []; // refresh filepaths
    } else {
        alert("select some image");
    }
}

window.addEventListener("DOMContentLoaded", () => {
    // splashscreen
    setTimeout(() => {
        invoke("close_splashscreen");
    }, 2000); // delay 2s before opening programe

    // home.html
    openFile = document.querySelector("#OpenFile");
    inputName = document.querySelector("#inputname");
    inputRoom = document.querySelector("#inputroom");
    processBtn = document.querySelector("#processBtn");

    // home.html
    openFile.addEventListener("click", (event) => {
        event.preventDefault();
        readFile() ;
    });
    
    processBtn.addEventListener("click", (event) => {
        event.preventDefault();
        processing();
    });
})