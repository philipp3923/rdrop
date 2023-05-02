export default function MatIcon({ name, fill, style="outlined", children, className, onClick, size, ...props  }) {
    switch(size){
        case "small":
        case "20": style += " s-20"; break;
        case "medium":
        case "32": style += " s-32"; break;
        case "semi":
        case "40": style += " s-40"; break;
        case "large":
        case "48": style += " s-48"; break;
    }
    if(fill) style += " fill";
    if(className) style += " " + className;

    if(props.left) style += " left";
    if(props.right) style += " right";

    return (
        <i className={"mat-icon material-symbols-" + style} onClick={onClick}>{children || name}</i>
    )
}