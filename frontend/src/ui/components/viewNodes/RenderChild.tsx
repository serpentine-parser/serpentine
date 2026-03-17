import { Node } from '@domains/graph/model/types';
import type { NodeInteractionProps } from './nodeInteraction';
import Class from './Class';
import Function from './Function';
import Module from './Module';

const RenderChild = (parent: Node, child: Node, interaction: NodeInteractionProps) => {
    switch (child.type) {
      case 'function':
        return <Function key={child.id} {...child} parentId={parent.id} {...interaction} />;
      case 'class':
        return <Class key={child.id} {...child} parentId={parent.id} {...interaction} />;
      case 'module':
        return <Module key={child.id} {...child} parentId={parent.id} {...interaction} />;
      default:
        return <Function key={child.id} {...child} parentId={parent.id} {...interaction} />;
    }
  };

export default RenderChild;
