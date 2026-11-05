function Hello(props: { name: string }) { return <div>{props.name}</div>; }
const App = () => <Hello name="x" />;
interface Props { name: string }
