export default function Button({ children, className, ...props }) {
    let classes = ["button"];
    if (props.outlined) classes.push("outlined");
    if (props.outlinedBig) classes.push("outlined-big");
    if (props.tonal) classes.push("tonal");
    if (props.text) classes.push("text");
    if (props.large) classes.push("large");
    if (className) classes.push(props.className);

    return (
        <button className={classes.join(" ")} {...props}>
            {children}
        </button>
    );
}