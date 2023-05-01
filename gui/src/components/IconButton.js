import MatIcon from "./MatIcon";

export default function IconButton({ name, children, ...props }) {
    let classes = ["icon-button"];
    if (props.outlined) classes.push("outlined");
    if (props.outlinedBig) classes.push("outlined-big");
    if (props.tonal) classes.push("tonal");
    if (props.text) classes.push("text");
    if (props.className) classes.push(props.className);

    return (
        <button className={classes.join(" ")} {...props}>
            <MatIcon name={name || children} fill={props.fill}/>
        </button>
    );
}