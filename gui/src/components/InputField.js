import { forwardRef, useEffect, useRef, useState } from "react";
import InputAlert from "./InputAlert";

export default forwardRef(({ control, error, description, onChange, onBlur, name, label, toggle, defaultValue, value, ...props }, ref) => {
    const [hasValue, setHasValue] = useState(defaultValue !== undefined && defaultValue.length > 0);
    const [show, setShow] = useState(false);

    const handleChange = (e) => {
        onChange && onChange(e);
        setHasValue(e.target.value.length > 0);
    }

    const handleBlur = (e) => {
        onBlur && onBlur(e);
        setHasValue(e.target.value.length > 0);
    }

    const handleFocus = () => {
        setHasValue(true);
        if(ref && ref.current !== undefined) ref.current.focus();
    }

    const hasToggle = props.type === "password" && toggle != false;
    const type = hasToggle ? (show ? "text" : "password") : props.type;

    if(error && error.message !== undefined) error = error.message;

    return <div className="input-field-container">
        <div className={"input-field" + (hasValue ? " active" : "") + (error ? " error" : "")} onClick={handleFocus}>
            <div className="input-field-content">
                {props.type !== "textarea" && <input placeholder=" " {...props} className="input-field-input" onChange={handleChange} onBlur={handleBlur} onFocus={handleFocus} ref={ref} name={name} id={name} type={type} defaultValue={defaultValue} />}
                {props.type === "textarea" && <textarea placeholder=" " rows="20" {...props} className="input-field-input" onChange={handleChange} onBlur={handleBlur} onFocus={handleFocus} ref={ref} name={name} id={name} defaultValue={defaultValue} />}
                {hasToggle && <div className="input-field-toggle no-select" onClick={() => setShow(!show)}>
                    {show ? <FormattedMessage {...login.hide} /> : <FormattedMessage {...login.show} />}
                </div>}
            </div>
            <label className="input-field-label layout-center-height" htmlFor={name}>{label}</label>
        </div>
        {description !== undefined && <p className="input-field-description">
            {description}
        </p>}
        {error !== undefined && error !== false && <div className="input-field-error">
            <InputAlert>{error}</InputAlert>
        </div>}
    </div>
})