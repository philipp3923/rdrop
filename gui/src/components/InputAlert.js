import MatIcon from "./MatIcon";

export default function InputAlert({ children, ...props }) {
    return <div className="input-alert" {...props} role="alert">
        <span className="m-r-8">
            <MatIcon name="error" small/>
        </span>
        {children}
    </div>
}